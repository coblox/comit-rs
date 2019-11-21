use crate::{
    db,
    swap_protocols::rfc003::{self, actions::ActionKind, state_store},
};
use http::StatusCode;
use http_api_problem::HttpApiProblem;
use libp2p_comit::frame::Response;
use serde::Serialize;
use warp::{Rejection, Reply};

#[derive(Debug, Serialize)]
pub struct MissingQueryParameter {
    pub name: &'static str,
    pub data_type: &'static str,
    pub description: &'static str,
}

pub fn from_anyhow(e: anyhow::Error) -> HttpApiProblem {
    if let Some(db::Error::SwapNotFound) = e.downcast_ref::<db::Error>() {
        return swap_not_found();
    }

    internal_error(e)
}

pub fn internal_error(e: anyhow::Error) -> HttpApiProblem {
    log::error!("internal error occurred: {:?}", e);
    HttpApiProblem::with_title_and_type_from_status(StatusCode::INTERNAL_SERVER_ERROR)
}

pub fn missing_channel() -> HttpApiProblem {
    log::error!("Channel for swap was not found in hash map");
    HttpApiProblem::with_title_and_type_from_status(StatusCode::INTERNAL_SERVER_ERROR)
}

pub fn send_over_channel(_e: Response) -> HttpApiProblem {
    log::error!("Sending response over channel failed");
    HttpApiProblem::with_title_and_type_from_status(StatusCode::INTERNAL_SERVER_ERROR)
}

pub fn state_store() -> HttpApiProblem {
    log::error!("State store didn't have state in it despite swap being in database");
    HttpApiProblem::with_title_and_type_from_status(StatusCode::INTERNAL_SERVER_ERROR)
}

pub fn swap_not_found() -> HttpApiProblem {
    HttpApiProblem::new("Swap not found.").set_status(StatusCode::NOT_FOUND)
}

pub fn unsupported() -> HttpApiProblem {
    HttpApiProblem::new("Swap not supported.")
        .set_status(StatusCode::BAD_REQUEST)
        .set_detail("The requested combination of ledgers and assets is not supported.")
}

pub fn deserialize(e: serde_json::Error) -> HttpApiProblem {
    log::error!("Failed to deserialize body: {:?}", e);
    HttpApiProblem::new("Invalid body.")
        .set_status(StatusCode::BAD_REQUEST)
        .set_detail("Failed to deserialize given body.")
}

pub fn serialize(e: serde_json::Error) -> HttpApiProblem {
    log::error!("Failed to serialize body: {:?}", e);
    HttpApiProblem::with_title_and_type_from_status(StatusCode::INTERNAL_SERVER_ERROR)
}

pub fn not_yet_implemented(feature: &str) -> HttpApiProblem {
    log::error!("{} not yet implemented", feature);
    HttpApiProblem::new("Feature not yet implemented.")
        .set_status(StatusCode::INTERNAL_SERVER_ERROR)
        .set_detail(format!("{} is not yet implemented! Sorry :(", feature))
}

pub fn action_already_done(action: ActionKind) -> HttpApiProblem {
    log::error!("{} action has already been done", action);
    HttpApiProblem::new("Action already done.").set_status(StatusCode::GONE)
}

pub fn invalid_action(action: ActionKind) -> HttpApiProblem {
    log::error!("{} action is invalid for this swap", action);
    HttpApiProblem::new("Invalid action.")
        .set_status(StatusCode::CONFLICT)
        .set_detail("Cannot perform requested action for this swap.")
}

pub fn unexpected_query_parameters(action: &str, parameters: Vec<String>) -> HttpApiProblem {
    log::error!(
        "Unexpected GET parameters {:?} for a {} action type. Expected: none",
        parameters,
        action
    );
    let mut problem = HttpApiProblem::new("Unexpected query parameter(s).")
        .set_status(StatusCode::BAD_REQUEST)
        .set_detail("This action does not take any query parameters.");

    problem
        .set_value("unexpected_parameters", &parameters)
        .expect("parameters will never fail to serialize");

    problem
}

pub fn missing_query_parameters(
    action: &str,
    parameters: Vec<&MissingQueryParameter>,
) -> HttpApiProblem {
    log::error!(
        "Missing GET parameters for a {} action type. Expected: {:?}",
        action,
        parameters
            .iter()
            .map(|parameter| parameter.name)
            .collect::<Vec<&str>>()
    );

    let mut problem = HttpApiProblem::new("Missing query parameter(s).")
        .set_status(StatusCode::BAD_REQUEST)
        .set_detail("This action requires additional query parameters.");

    problem
        .set_value("missing_parameters", &parameters)
        .expect("parameters will never fail to serialize");

    problem
}

impl From<state_store::Error> for HttpApiProblem {
    fn from(e: state_store::Error) -> Self {
        log::error!("Storage layer failure: {:?}", e);
        HttpApiProblem::with_title_and_type_from_status(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

impl From<rfc003::state_machine::Error> for HttpApiProblem {
    fn from(e: rfc003::state_machine::Error) -> Self {
        log::error!("Protocol execution error: {:?}", e);
        HttpApiProblem::with_title_and_type_from_status(StatusCode::INTERNAL_SERVER_ERROR)
            .set_title("Protocol execution error.")
    }
}

pub fn unpack_problem(rejection: Rejection) -> Result<impl Reply, Rejection> {
    if let Some(problem) = rejection.find_cause::<HttpApiProblem>() {
        log::debug!(target: "http-api", "HTTP request got rejected, returning HttpApiProblem response: {:?}", problem);

        let code = problem.status.unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

        let reply = warp::reply::json(problem);
        let reply = warp::reply::with_status(reply, code);
        let reply = warp::reply::with_header(
            reply,
            http::header::CONTENT_TYPE,
            http_api_problem::PROBLEM_JSON_MEDIA_TYPE,
        );

        return Ok(reply);
    }

    Err(rejection)
}
