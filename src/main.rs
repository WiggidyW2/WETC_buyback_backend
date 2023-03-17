mod proto {
    tonic::include_proto!("weve_esi_proto");
}
mod static_map;
mod response;
mod pricing;
mod error;
mod parse;
mod item;
mod io;

use pricing::{PricingModel, Price};
use io::{ParsedInput, read_stdin};
use response::Response;
use error::Error;
use item::Item;

use futures::stream::{TryStreamExt, futures_unordered::FuturesUnordered};
use firestore::{*, errors::*};
use gcloud_sdk;

type Client = proto::weve_market_client::WeveMarketClient<
    tonic::transport::Channel
>;
type Location = &'static str;
type Market = &'static str;
type Hash<'s> = &'s str;
type ItemName = String;
type Quantity = f64;
type PriceMod = f64;
type TypeId = i32;

#[tokio::main]
async fn main() {
    let mut buf: String = String::new();
    read_stdin(&mut buf).unwrap();

    let parsed_input: ParsedInput = ParsedInput::from_str(&buf).unwrap();
    let response: Response = match parsed_input {
        ParsedInput::Items((v, l)) => response_from_items(v, l).await.unwrap(),
        ParsedInput::Hash(s) => response_from_hash(s).await.unwrap(),
    };

    response.to_stdout().unwrap();
}

async fn response_from_items(
    items: Vec<(Item, PricingModel)>,
    location: &str,
) -> Result<Response, Error> {
    let mut response: Response = Response::with_capacity(
        items.len(),
        location.to_string(),
    );

    let mut return_empty: bool = true;
    for item in &items {
        if &item.1 != &PricingModel::Rejected {
            return_empty = false;
            break;
        }
    }
    if return_empty {
        for item in items {
            response.push(item.0, Price::Rejected);
        }
        return Ok(response);
    }

    let db = get_db();
    let dst = io::read_dst()?;
    let client: Client = Client::connect(dst)
        .await
        .map_err(|e| Error::GRPCConnectionError(e))?;

    let mut stream = items
        .into_iter()
        .map(|(item, model)| get_price(item, model, client.clone()))
        .collect::<FuturesUnordered<_>>();
    while let Some((item, price)) = stream
        .try_next()
        .await?
    {
        response.push(item, price);
    }
    response.sort();
    let hash_key: &str = response.with_hash_key();

    match db
        .await?
        .fluent()
        .insert()
        .into("hash_cache")
        .document_id(hash_key)
        .object(&response)
        .execute::<Response>()
        .await {
            Err(e) if is_error(&e) => Err(Error::FirestoreInsertError(e)),
            _ => Ok(response),
        }
}

async fn response_from_hash(hash_cache_key: &str) -> Result<Response, Error> {
    get_db()
        .await?
        .fluent()
        .select()
        .by_id_in("hash_cache")
        .obj()
        .one(hash_cache_key)
        .await
        .map(|o| o.unwrap_or(Response::with_capacity(0, "".to_string())))
        .map_err(|e| Error::FirestoreSelectError(e))
}

async fn get_price(
    item: Item,
    pricing_model: PricingModel,
    client: Client,
) -> Result<(Item, Price), Error> {
    pricing_model
        .get_price(client)
        .await
        .map(|p| (item, p))
}

async fn get_db() -> Result<FirestoreDb, Error> {
    FirestoreDb::with_options_token_source(
        FirestoreDbOptions::new(io::read_project_id().unwrap()),
        gcloud_sdk::GCP_DEFAULT_SCOPES.clone(),
        gcloud_sdk::TokenSourceType::Json(io::read_firestore_token().unwrap()),
    )
        .await
        .map_err(|e| Error::FirestoreConnectionError(e))
}

// Returns false if the error is "AlreadyExists"
fn is_error(err: &FirestoreError) -> bool {
    if let FirestoreError::DataConflictError(inner_err) = err {
        if &inner_err.public.code == "AlreadyExists" {
            return false
        }
    }
    true
}
