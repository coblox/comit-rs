use crate::{
    db::{DetermineTypes, SaveMessage, SaveRfc003Messages},
    http_api::{
        action::{
            ActionExecutionParameters, ActionResponseBody, IntoResponsePayload, ListRequiredFields,
            ToSirenAction,
        },
        problem,
        route_factory::new_action_link,
        routes::rfc003::decline::{to_swap_decline_reason, DeclineBody},
    },
    libp2p_comit_ext::ToHeader,
    network::Network,
    seed::SwapSeed,
    swap_protocols::{
        actions::Actions,
        rfc003::{
            self,
            actions::{Action, ActionKind},
            bob::State,
            messages::{Decision, IntoAcceptMessage},
            state_store::StateStore,
            Spawn,
        },
        SwapId,
    },
};
use futures::Stream;
use http_api_problem::HttpApiProblem;
use libp2p_comit::frame::Response;
use std::fmt::Debug;

#[allow(clippy::unit_arg, clippy::let_unit_value)]
pub fn handle_action<
    D: StateStore + Network + Spawn + SwapSeed + SaveRfc003Messages + DetermineTypes,
>(
    method: http::Method,
    swap_id: SwapId,
    action_kind: ActionKind,
    body: serde_json::Value,
    query_params: ActionExecutionParameters,
    dependencies: D,
) -> Result<ActionResponseBody, HttpApiProblem> {
    let types = dependencies
        .determine_types(&swap_id)
        .map_err(problem::internal_error)?;

    with_swap_types!(
        types,
        (|| {
            let state = StateStore::get::<ROLE>(&dependencies, &swap_id)?
                .ok_or_else(problem::state_store)?;
            log::trace!("Retrieved state for {}: {:?}", swap_id, state);

            state
                .actions()
                .into_iter()
                .select_action(action_kind, method)
                .and_then({
                    |action| match action {
                        Action::Accept(_) => serde_json::from_value::<AcceptBody>(body)
                            .map_err(problem::deserialize)
                            .and_then({
                                |body| {
                                    let channel =
                                        Network::pending_request_for(&dependencies, swap_id)
                                            .ok_or_else(problem::missing_channel)?;

                                    let accept_message = body.into_accept_message(
                                        swap_id,
                                        &SwapSeed::swap_seed(&dependencies, swap_id),
                                    );

                                    SaveMessage::save_message(&dependencies, accept_message)
                                        .map_err(problem::internal_error)?;

                                    let response = rfc003_accept_response(accept_message);
                                    channel.send(response).map_err(problem::send_over_channel)?;

                                    let swap_request = state.request();
                                    let seed = dependencies.swap_seed(swap_id);
                                    let state =
                                        State::accepted(swap_request.clone(), accept_message, seed);
                                    StateStore::insert(&dependencies, swap_id, state);

                                    let receiver =
                                        Spawn::spawn(&dependencies, swap_request, accept_message);

                                    tokio::spawn(receiver.for_each(move |update| {
                                        StateStore::update::<State<AL, BL, AA, BA>>(
                                            &dependencies,
                                            &swap_id,
                                            update,
                                        );
                                        Ok(())
                                    }));

                                    Ok(ActionResponseBody::None)
                                }
                            }),
                        Action::Decline(_) => serde_json::from_value::<DeclineBody>(body)
                            .map_err(problem::deserialize)
                            .and_then({
                                |body| {
                                    let channel =
                                        Network::pending_request_for(&dependencies, swap_id)
                                            .ok_or_else(problem::missing_channel)?;

                                    let decline_message = rfc003::Decline {
                                        swap_id,
                                        reason: to_swap_decline_reason(body.reason),
                                    };

                                    SaveMessage::save_message(
                                        &dependencies,
                                        decline_message.clone(),
                                    )
                                    .map_err(problem::internal_error)?;

                                    let response = rfc003_decline_response(decline_message.clone());
                                    channel.send(response).map_err(problem::send_over_channel)?;

                                    let swap_request = state.request();
                                    let seed = dependencies.swap_seed(swap_id);
                                    let state = State::declined(
                                        swap_request.clone(),
                                        decline_message.clone(),
                                        seed,
                                    );
                                    StateStore::insert(&dependencies, swap_id, state);

                                    Ok(ActionResponseBody::None)
                                }
                            }),
                        Action::Deploy(action) => action.into_response_payload(query_params),
                        Action::Fund(action) => action.into_response_payload(query_params),
                        Action::Redeem(action) => action.into_response_payload(query_params),
                        Action::Refund(action) => action.into_response_payload(query_params),
                    }
                })
        })
    )
}

trait SelectAction<Accept, Decline, Deploy, Fund, Redeem, Refund>:
    Iterator<Item = Action<Accept, Decline, Deploy, Fund, Redeem, Refund>>
{
    fn select_action(
        mut self,
        action_kind: ActionKind,
        method: http::Method,
    ) -> Result<Self::Item, HttpApiProblem>
    where
        Self: Sized,
    {
        self.find(|action| ActionKind::from(action) == action_kind)
            .ok_or_else(|| problem::invalid_action(action_kind))
            .and_then(|action| {
                if http::Method::from(action_kind) != method {
                    log::debug!(target: "http-api", "Attempt to invoke {} action with http method {}, which is an invalid combination.", action_kind, method);
                    return Err(HttpApiProblem::new("Invalid action invocation")
                        .set_status(http::StatusCode::METHOD_NOT_ALLOWED));
                }

                Ok(action)
            })
    }
}

fn rfc003_accept_response<AL: rfc003::Ledger, BL: rfc003::Ledger>(
    message: rfc003::messages::Accept<AL, BL>,
) -> Response {
    Response::empty()
        .with_header(
            "decision",
            Decision::Accepted
                .to_header()
                .expect("Decision should not fail to serialize"),
        )
        .with_body(
            serde_json::to_value(rfc003::messages::AcceptResponseBody::<AL, BL> {
                beta_ledger_refund_identity: message.beta_ledger_refund_identity,
                alpha_ledger_redeem_identity: message.alpha_ledger_redeem_identity,
            })
            .expect("body should always serialize into serde_json::Value"),
        )
}

fn rfc003_decline_response(message: rfc003::messages::Decline) -> Response {
    Response::empty()
        .with_header(
            "decision",
            Decision::Declined
                .to_header()
                .expect("Decision shouldn't fail to serialize"),
        )
        .with_body(
            serde_json::to_value(rfc003::messages::DeclineResponseBody {
                reason: message.reason,
            })
            .expect("decline body should always serialize into serde_json::Value"),
        )
}

impl<Accept, Decline, Deploy, Fund, Redeem, Refund, I>
    SelectAction<Accept, Decline, Deploy, Fund, Redeem, Refund> for I
where
    I: Iterator<Item = Action<Accept, Decline, Deploy, Fund, Redeem, Refund>>,
{
}

#[cfg(test)]
mod tests {

    use super::*;
    use spectral::prelude::*;

    fn actions() -> Vec<Action<(), (), (), (), (), ()>> {
        Vec::new()
    }

    #[test]
    fn action_not_available_should_return_409_conflict() {
        let given_actions = actions();

        let result = given_actions
            .into_iter()
            .select_action(ActionKind::Accept, http::Method::POST);

        assert_that(&result)
            .is_err()
            .map(|p| &p.status)
            .is_equal_to(Some(http::StatusCode::CONFLICT));
    }

    #[test]
    fn accept_decline_action_should_be_returned_with_http_post() {
        let mut given_actions = actions();
        given_actions.extend(vec![Action::Accept(()), Action::Decline(())]);

        let result = given_actions
            .clone()
            .into_iter()
            .select_action(ActionKind::Accept, http::Method::POST);

        assert_that(&result).is_ok_containing(Action::Accept(()));

        let result = given_actions
            .clone()
            .into_iter()
            .select_action(ActionKind::Decline, http::Method::POST);

        assert_that(&result).is_ok_containing(Action::Decline(()));
    }

    #[test]
    fn accept_decline_action_cannot_be_invoked_with_http_get() {
        let mut given_actions = actions();
        given_actions.extend(vec![Action::Accept(()), Action::Decline(())]);

        let result = given_actions
            .clone()
            .into_iter()
            .select_action(ActionKind::Accept, http::Method::GET);

        assert_that(&result)
            .is_err()
            .map(|p| &p.status)
            .is_equal_to(Some(http::StatusCode::METHOD_NOT_ALLOWED));

        let result = given_actions
            .clone()
            .into_iter()
            .select_action(ActionKind::Decline, http::Method::GET);

        assert_that(&result)
            .is_err()
            .map(|p| &p.status)
            .is_equal_to(Some(http::StatusCode::METHOD_NOT_ALLOWED));
    }

    #[test]
    fn deploy_fund_refund_redeem_action_cannot_be_invoked_with_http_post() {
        let mut given_actions = actions();
        given_actions.extend(vec![
            Action::Deploy(()),
            Action::Fund(()),
            Action::Refund(()),
            Action::Redeem(()),
        ]);

        let result = given_actions
            .clone()
            .into_iter()
            .select_action(ActionKind::Deploy, http::Method::POST);

        assert_that(&result)
            .is_err()
            .map(|p| &p.status)
            .is_equal_to(Some(http::StatusCode::METHOD_NOT_ALLOWED));

        let result = given_actions
            .clone()
            .into_iter()
            .select_action(ActionKind::Fund, http::Method::POST);

        assert_that(&result)
            .is_err()
            .map(|p| &p.status)
            .is_equal_to(Some(http::StatusCode::METHOD_NOT_ALLOWED));

        let result = given_actions
            .clone()
            .into_iter()
            .select_action(ActionKind::Refund, http::Method::POST);

        assert_that(&result)
            .is_err()
            .map(|p| &p.status)
            .is_equal_to(Some(http::StatusCode::METHOD_NOT_ALLOWED));

        let result = given_actions
            .clone()
            .into_iter()
            .select_action(ActionKind::Redeem, http::Method::POST);

        assert_that(&result)
            .is_err()
            .map(|p| &p.status)
            .is_equal_to(Some(http::StatusCode::METHOD_NOT_ALLOWED));
    }
}

impl From<ActionKind> for http::Method {
    fn from(action_kind: ActionKind) -> Self {
        match action_kind {
            ActionKind::Accept => http::Method::POST,
            ActionKind::Decline => http::Method::POST,
            ActionKind::Deploy => http::Method::GET,
            ActionKind::Fund => http::Method::GET,
            ActionKind::Refund => http::Method::GET,
            ActionKind::Redeem => http::Method::GET,
        }
    }
}

impl<Accept, Decline, Deploy, Fund, Redeem, Refund> IntoResponsePayload
    for Action<Accept, Decline, Deploy, Fund, Redeem, Refund>
where
    Deploy: IntoResponsePayload,
    Fund: IntoResponsePayload,
    Redeem: IntoResponsePayload,
    Refund: IntoResponsePayload,
{
    fn into_response_payload(
        self,
        query_params: ActionExecutionParameters,
    ) -> Result<ActionResponseBody, HttpApiProblem> {
        match self {
            Action::Deploy(payload) => payload.into_response_payload(query_params),
            Action::Fund(payload) => payload.into_response_payload(query_params),
            Action::Redeem(payload) => payload.into_response_payload(query_params),
            Action::Refund(payload) => payload.into_response_payload(query_params),
            Action::Accept(_) | Action::Decline(_) => {
                log::error!(target: "http-api", "IntoResponsePayload is not available for Accept/Decline");
                Err(HttpApiProblem::with_title_and_type_from_status(
                    http::StatusCode::INTERNAL_SERVER_ERROR,
                ))
            }
        }
    }
}

impl<Accept, Decline, Deploy, Fund, Redeem, Refund> ToSirenAction
    for Action<Accept, Decline, Deploy, Fund, Redeem, Refund>
where
    Accept: ListRequiredFields + Debug,
    Decline: ListRequiredFields + Debug,
    Deploy: ListRequiredFields + Debug,
    Fund: ListRequiredFields + Debug,
    Redeem: ListRequiredFields + Debug,
    Refund: ListRequiredFields + Debug,
{
    fn to_siren_action(&self, id: &SwapId) -> siren::Action {
        let action_kind = ActionKind::from(self);
        let method = http::Method::from(action_kind);
        let name = action_kind.to_string();

        let media_type = match method {
            // GET + DELETE cannot have a body
            http::Method::GET | http::Method::DELETE => None,
            _ => Some("application/json".to_owned()),
        };

        let fields = match self {
            Action::Accept(_) => Accept::list_required_fields(),
            Action::Decline(_) => Decline::list_required_fields(),
            Action::Deploy(_) => Deploy::list_required_fields(),
            Action::Fund(_) => Fund::list_required_fields(),
            Action::Redeem(_) => Redeem::list_required_fields(),
            Action::Refund(_) => Refund::list_required_fields(),
        };

        log::debug!(target: "http-api", "Creating siren::Action from {:?} with HTTP method: {}, Media-Type: {:?}, Name: {}, Fields: {:?}", self, method, media_type, name, fields);

        siren::Action {
            href: new_action_link(id, &name),
            name,
            method: Some(method),
            _type: media_type,
            fields,
            class: vec![],
            title: None,
        }
    }
}
