use crate::{
    {TypeId, Market, Quantity, Client, PriceMod},
    static_map::MAX_MULTI_ITEM,
    error::Error,
    proto::*,
};

use futures::stream::{TryStreamExt, futures_unordered::FuturesUnordered};

#[derive(Debug, Clone, Copy)]
pub enum Price {
    Accepted(f64),
    Rejected,
}

#[derive(Debug, Clone)]
pub enum PricingModel {
    SingleMarketSingleItemMaxBuy(SingleMarketSingleItemMaxBuy),
    SingleMarketMultiItemMaxBuy(SingleMarketMultiItemMaxBuy),
    Rejected,
}

#[derive(Debug, Clone)]
pub struct SingleMarketSingleItemMaxBuy(pub TypeId, pub Market, pub PriceMod);
#[derive(Debug, Clone)]
pub struct SingleMarketMultiItemMaxBuy(
    pub [Option<(TypeId, Quantity)>; MAX_MULTI_ITEM],
    pub Market,
    pub PriceMod,
);

trait WeveMarketMessages {
    fn to_reqs(&self) -> Vec<MarketOrdersReq>;
    fn get_price(
        &self,
        reps: Vec<(MarketOrdersReq, MarketOrdersRep)>,
    ) -> Price;
}

impl PricingModel {
    pub async fn get_price(&self, client: Client) -> Result<Price, Error> {
        let reps: Vec<(MarketOrdersReq, MarketOrdersRep)> = match self {
            PricingModel::SingleMarketSingleItemMaxBuy(p) => p.to_reqs(),
            PricingModel::SingleMarketMultiItemMaxBuy(p) => p.to_reqs(),
            PricingModel::Rejected => return Ok(Price::Rejected),
        }
            .into_iter()
            .map(|req| get_market_orders(client.clone(), req))
            .collect::<FuturesUnordered<_>>()
            .try_collect() // Do not try to type-annotate this
            .await?;
        
        Ok(match self {
            PricingModel::SingleMarketSingleItemMaxBuy(p) => p.get_price(reps),
            PricingModel::SingleMarketMultiItemMaxBuy(p) => p.get_price(reps),
            PricingModel::Rejected => unreachable!(),
        })
    }
}

impl WeveMarketMessages for SingleMarketSingleItemMaxBuy {
    fn to_reqs(&self) -> Vec<MarketOrdersReq> {
        vec![MarketOrdersReq {
            type_id: self.0,
            market: self.1.to_string(),
            buy: true,
        }]
    }
    fn get_price(
        &self,
        reps: Vec<(MarketOrdersReq, MarketOrdersRep)>,
    ) -> Price {
        match reps
            .into_iter()
            .next()
            .unwrap() // If this were None, we would have returned an error in async get_price
            .1
            .market_orders
            .into_iter()
            .max_by(|o1, o2| order_f64(&o1.price, &o2.price))
        {
            Some(order) => Price::Accepted(order.price * self.2),
            None => Price::Rejected, // This is when there are no orders
        }
    }
}

impl WeveMarketMessages for SingleMarketMultiItemMaxBuy {
    fn to_reqs(&self) -> Vec<MarketOrdersReq> {
        self.0
            .iter()
            .filter_map(
                |option| option.map(
                    |(type_id, _)| MarketOrdersReq {
                        type_id: type_id,
                        market: self.1.to_string(),
                        buy: true,
                    }
                )
            )
            .collect()
    }
    fn get_price(
        &self,
        reps: Vec<(MarketOrdersReq, MarketOrdersRep)>,
    ) -> Price {
        let mut price: f64 = 0.0;
        for (req, rep) in reps {
            for (type_id, quantity) in self
                .0
                .iter()
                .filter_map(|option| option.as_ref())
            {
                if &req.type_id == type_id {
                    match rep
                        .market_orders
                        .iter()
                        .max_by(|o1, o2| order_f64(&o1.price, &o2.price))
                    {
                        Some(order) => price += order.price * quantity,
                        None => return Price::Rejected,
                    }
                    continue;
                }
            }
        }
        Price::Accepted(price * self.2)
    }
}

async fn get_market_orders(
    mut client: Client,
    req: MarketOrdersReq,
) -> Result<(MarketOrdersReq, MarketOrdersRep), Error> {
    Ok((req.clone(), client
        .market_orders(req)
        .await?
        .into_inner()
    ))
}

fn order_f64(v1: &f64, v2: &f64) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match v1.partial_cmp(v2) {
        Some(ordering) => ordering,
        None => {
            if v1.is_finite() && v2.is_finite() {
                Ordering::Equal
            }
            else if v1.is_finite() {
                Ordering::Greater
            }
            else if v2.is_finite() {
                Ordering::Less
            }
            else {
                unreachable!()
            }
        },
    }
}
