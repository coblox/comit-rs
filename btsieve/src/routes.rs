use crate::{
    query_repository::QueryRepository,
    query_result_repository::{QueryResult, QueryResultRepository},
    route_factory::{QueryParams, ToHttpPayload, MAX_QUERY_ID_LENGTH},
};
use http::StatusCode;
use http_api_problem::HttpApiProblem;
use serde::{Deserialize, Serialize};
use std::{
    error::Error as StdError,
    fmt::{self, Debug},
    sync::Arc,
};
use warp::{self, Rejection, Reply};

#[derive(Debug)]
pub enum Error {
    QuerySave,
    TransformToPayload,
    QueryNotFound,
    NetworkNotFound,
    LedgerNotConnected,
    QueryMismatch,
    QueryIdTooLong,
}

#[derive(Debug)]
pub struct HttpApiProblemStdError {
    pub http_api_problem: HttpApiProblem,
}

impl fmt::Display for HttpApiProblemStdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.http_api_problem.title)
    }
}

impl StdError for HttpApiProblemStdError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        None
    }
}

impl From<Error> for HttpApiProblem {
    fn from(e: Error) -> Self {
        use self::Error::*;
        match e {
            QuerySave => {
                HttpApiProblem::with_title_and_type_from_status(StatusCode::INTERNAL_SERVER_ERROR)
                    .set_detail("Failed to create new query.")
            }
            TransformToPayload => {
                HttpApiProblem::with_title_and_type_from_status(StatusCode::INTERNAL_SERVER_ERROR)
            }
            QueryNotFound => HttpApiProblem::with_title_and_type_from_status(StatusCode::NOT_FOUND)
                .set_detail("The requested query does not exist."),
            NetworkNotFound => {
                HttpApiProblem::with_title_and_type_from_status(StatusCode::NOT_FOUND)
                    .set_detail("The requested network id does not exist.")
            }
            LedgerNotConnected => {
                HttpApiProblem::with_title_and_type_from_status(StatusCode::SERVICE_UNAVAILABLE)
                    .set_detail("The requested ledger is not connected.")
            }
            QueryMismatch => HttpApiProblem::with_title_and_type_from_status(StatusCode::CONFLICT)
                .set_detail("The query exists but is not equivalent to the request."),
            QueryIdTooLong => HttpApiProblem::with_title_and_type_from_status(
                StatusCode::BAD_REQUEST,
            )
            .set_detail(format!(
                "The query ID supplied exceeds the {} character limit.",
                MAX_QUERY_ID_LENGTH
            )),
        }
    }
}

pub fn customize_error(rejection: Rejection) -> Result<impl Reply, Rejection> {
    if let Some(err) = rejection.find_cause::<HttpApiProblemStdError>() {
        let code = err
            .http_api_problem
            .status
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let json = warp::reply::json(&err.http_api_problem);
        return Ok(warp::reply::with_status(json, code));
    }
    Err(rejection)
}

#[allow(clippy::needless_pass_by_value)]
pub fn retrieve_query<
    R: Debug + Default,
    Q: Serialize + Send + Debug,
    QR: QueryRepository<Q>,
    QRR: QueryResultRepository<Q>,
    C: 'static + Send + Sync,
>(
    client: Arc<C>,
    _network: String,
    query_repository: Arc<QR>,
    query_result_repository: Arc<QRR>,
    id: String,
    query_params: QueryParams<R>,
) -> Result<impl Reply, Rejection>
where
    for<'de> R: Deserialize<'de>,
    QueryResult: ToHttpPayload<R, Client = C>,
{
    query_repository
        .get(id.clone())
        .ok_or(Error::QueryNotFound)
        .and_then(|query| {
            query_result_repository
                .get(id.clone())
                .unwrap_or_default()
                .to_http_payload(&query_params.return_as, &client)
                .map(|matches| RetrieveQueryResponse { query, matches })
                .map(|response| warp::reply::json(&response))
                .map_err(|e| {
                    log::error!(
                        "failed to transform result for query {} to payload {:?}: {:?}.",
                        id,
                        query_params.return_as,
                        e
                    );
                    Error::TransformToPayload
                })
        })
        .map_err(|e| {
            warp::reject::custom(HttpApiProblemStdError {
                http_api_problem: e.into(),
            })
        })
}

#[allow(clippy::needless_pass_by_value)]
pub fn delete_query<
    Q: Send,
    QR: QueryRepository<Q>,
    QRR: QueryResultRepository<Q>,
    C: 'static + Send + Sync,
>(
    _client: Arc<C>,
    _network: String,
    query_repository: Arc<QR>,
    query_result_repository: Arc<QRR>,
    id: String,
) -> Result<impl Reply, Rejection> {
    query_repository.delete(id.clone());
    query_result_repository.delete(id);

    Ok(warp::reply::with_status(
        warp::reply(),
        warp::http::StatusCode::NO_CONTENT,
    ))
}

#[allow(clippy::needless_pass_by_value)]
pub fn get_or_create_query<Q: Send + Eq, QR: QueryRepository<Q>, C: 'static + Send + Sync>(
    _client: Arc<C>,
    _network: String,
    query_repository: Arc<QR>,
    id: String,
    query: Q,
) -> Result<impl Reply, Rejection> {
    match query_repository.get(id.clone()) {
        Some(ref saved_query) if saved_query == &query => Ok(warp::reply::with_status(
            warp::reply(),
            warp::http::StatusCode::OK,
        )),
        Some(_) => Err(warp::reject::custom(HttpApiProblemStdError {
            http_api_problem: Error::QueryMismatch.into(),
        })),
        None => {
            let result = query_repository.save_with_id(query, id);

            match result {
                Ok(_) => Ok(warp::reply::with_status(
                    warp::reply(),
                    warp::http::StatusCode::NO_CONTENT,
                )),
                Err(_) => Err(warp::reject::custom(HttpApiProblemStdError {
                    http_api_problem: Error::QuerySave.into(),
                })),
            }
        }
    }
}

#[derive(Debug, Serialize, Clone, Default)]
pub struct RetrieveQueryResponse<Q, T> {
    query: Q,
    matches: T,
}
