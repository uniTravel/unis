// use crate::domain::Command;
// use axum::{body::Body, extract::FromRequest, http::StatusCode};
// use bytes::Bytes;

// pub struct Request<T>(pub T);

// impl<T, S> FromRequest<S> for Request<T>
// where
//     T: Command,
//     S: Send + Sync,
// {
//     type Rejection = (StatusCode, String);

//     async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self, Self::Rejection> {
//         let bytes = Bytes::from_request(req, state)
//             .await
//             .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;


//         // Ok(Request(value))
//         todo!()
//     }
// }

// // where
// //     T: Command + bincode::Decode,
// //     S: Send + Sync,
// // {
// //     type Rejection = (StatusCode, String);

// //     async fn from_request(
// //         req: axum::http::Request<Body>,
// //         state: &S,
// //     ) -> Result<Self, Self::Rejection> {
// //         let bytes = Bytes::from_request(req, state)
// //             .await
// //             .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

// //         let value: T = bincode::decode_from_slice(&bytes, BINCODE_CONFIG)
// //             .map(|(value, _)| value)
// //             .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

// //         todo!()
// //     }
// // }
