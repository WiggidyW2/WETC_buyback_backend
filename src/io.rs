use crate::{
    pricing::PricingModel,
    static_map::PM_MAP,
    parse::parse,
    error::Error,
    item::Item,
    Hash,
};

use std::env::var;

use serde::Deserialize;
use serde_json;
use tonic;

pub fn read_dst() -> Result<
    impl std::convert::TryInto<
        tonic::transport::Endpoint,
        Error = impl Into<tonic::codegen::StdError>,
    > + Clone,
    Error,
> {
    Ok(var("BBBE_WEVEMARKET")?)
}

pub fn read_project_id() -> Result<String, Error> {
    Ok(var("BBBE_GCPPROJECTID")?)
}

pub fn read_firestore_token() -> Result<String, Error> {
    Ok(var("BBBE_GCPTOKEN")?)
}

pub enum ParsedInput<'s> {
    Items((Vec<(Item, PricingModel)>, &'s str)),
    Hash(Hash<'s>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum Input<'s> {
    #[serde(bound(deserialize = "HashInput<'s>: Deserialize<'de>"))]
    HashInput(HashInput<'s>),
    #[serde(bound(deserialize = "ItemInput<'s>: Deserialize<'de>"))]
    ItemInput(ItemInput<'s>),
}

#[derive(Debug, Clone, Deserialize)]
struct HashInput<'s> {
    hash: &'s str, // This needs to be &str instead of Hash because of serde
}

#[derive(Debug, Clone, Deserialize)]
struct ItemInput<'s> {
    location: &'s str,
    #[serde(flatten)]
    items: ItemInputItems,
}

#[derive(Debug, Clone, Deserialize)]
enum ItemInputItems {
    #[serde(rename = "items")]
    Json(Vec<Item>),
    #[serde(rename = "raw")]
    Raw(String),
}

impl<'s> ParsedInput<'s> {
    pub fn from_str(s: &'s str) -> Result<ParsedInput<'s>, Error> {
        let input: Input<'s> = serde_json::from_str(s)
            .map_err(|e| Error::DeserializationError(e))?;
        ParsedInput::try_from_input(input)
    }

    pub fn from_slice(b: &'s [u8]) -> Result<ParsedInput<'s>, Error> {
        let input: Input<'s> = serde_json::from_slice(b)
            .map_err(|e| Error::DeserializationError(e))?;
        ParsedInput::try_from_input(input)
    }

    fn try_from_input(input: Input<'s>) -> Result<ParsedInput<'s>, Error> {
        let (location, items): (&str, Vec<Item>) = match input {
            Input::HashInput(h) => return Ok(ParsedInput::Hash(h.hash)),
            Input::ItemInput(i) => match i.items {
                ItemInputItems::Json(v) => (
                    i.location,
                    v,
                ),
                ItemInputItems::Raw(s) => (
                    i.location,
                    parse(&s)?,
                ),
            }
        };

        let mut inner: Vec<(Item, PricingModel)> = Vec::with_capacity(
            items.len()
        );
        for item in items.into_iter() {
            let entry: (Item, PricingModel) = match PM_MAP
                .get(location) 
                .map(|lmap| lmap.get(&item.name))
            {
                Some(Some(pricing_model)) => (item, pricing_model.clone()),
                Some(None) | None => (item, PricingModel::Rejected),
            };
            inner.push(entry);
        }

        Ok(ParsedInput::Items((inner, location)))
    }
}
