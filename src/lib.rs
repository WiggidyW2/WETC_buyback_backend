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

pub use response::Response;
pub use io::ParsedInput;
pub use error::Error;

use pricing::{PricingModel, Price};
use item::Item;

use futures::stream::{TryStreamExt, futures_unordered::FuturesUnordered};
use firestore::{*, errors::*};
use gcloud_sdk;

pub type Client = proto::weve_market_client::WeveMarketClient<
    tonic::transport::Channel
>;
type PriceSource = &'static str;
type Location = &'static str;
type Market = &'static str;
type Hash<'s> = &'s str;
type ItemName = String;
type Quantity = f64;
type PriceMod = f64;
type TypeId = i32;

pub async fn response_from_items(
    items: Vec<(Item, PricingModel)>,
    location: &str,
    db: &FirestoreDb,
    client: &Client,
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
            response.push(item.0, Price::Rejected, item.1.price_source());
        }
        return Ok(response);
    }

    let mut stream = items
        .into_iter()
        .map(|(item, model)| get_price(item, model, client.clone()))
        .collect::<FuturesUnordered<_>>();
    while let Some((item, price, price_source)) = stream
        .try_next()
        .await?
    {
        response.push(item, price, price_source);
    }
    response.sort();
    let hash_key: &str = response.with_hash_key();

    match db
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

pub async fn shell_response_from_items(
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
            response.push(item.0, Price::Rejected, item.1.price_source());
        }
        return Ok(response);
    }

    let db = get_db(); // Unawaited Future
    let client: Client = get_client().await?;

    let mut stream = items
        .into_iter()
        .map(|(item, model)| get_price(item, model, client.clone()))
        .collect::<FuturesUnordered<_>>();
    while let Some((item, price, price_source)) = stream
        .try_next()
        .await?
    {
        response.push(item, price, price_source);
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

pub async fn response_from_hash(
    hash_cache_key: &str,
    db: &FirestoreDb,
) -> Result<Response, Error> {
    db
        .fluent()
        .select()
        .by_id_in("hash_cache")
        .obj()
        .one(hash_cache_key)
        .await
        .map(|o| o.unwrap_or(Response::with_capacity(0, "".to_string())))
        .map_err(|e| Error::FirestoreSelectError(e))
}

pub async fn shell_response_from_hash(
    hash_cache_key: &str,
) -> Result<Response, Error> {
    response_from_hash(
        hash_cache_key,
        &get_db().await?,
    ).await
}

pub async fn get_client() -> Result<Client, Error> {
    Client::connect(
        io::read_dst()?
    )
        .await
        .map_err(|e| Error::GRPCConnectionError(e))
}

pub async fn get_db() -> Result<FirestoreDb, Error> {
    FirestoreDb::with_options_token_source(
        FirestoreDbOptions::new(io::read_project_id().unwrap()),
        gcloud_sdk::GCP_DEFAULT_SCOPES.clone(),
        gcloud_sdk::TokenSourceType::Json(io::read_firestore_token().unwrap()),
    )
        .await
        .map_err(|e| Error::FirestoreConnectionError(e))
}

async fn get_price(
    item: Item,
    pricing_model: PricingModel,
    client: Client,
) -> Result<(Item, Price, PriceSource), Error> {
    pricing_model
        .get_price(client)
        .await
        .map(|p| (item, p, pricing_model.price_source()))
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
