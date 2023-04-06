use crate::{
    {TypeId, Market, Quantity, Client, PriceMod, PriceSource, Location},
    static_map::{PM_MAP, MAX_MULTI_ITEM, MAX_SUB_ITEM},
    error::Error,
    proto::*,
};

use futures::stream::{TryStreamExt, futures_unordered::FuturesUnordered};

#[derive(Debug, Clone, Copy)]
pub enum Price {
    Accepted(f64),
    Rejected,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PricingModel {
    SingleMarketSingleItemMaxBuy(SingleMarketSingleItemMaxBuy),
    SingleMarketMultiItemMaxBuy(SingleMarketMultiItemMaxBuy),
    SubSingleItemsMaxBuy(SubSingleItemsMaxBuy),
    Rejected,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SingleMarketSingleItemMaxBuy(
    pub TypeId,
    pub Market,
    pub PriceMod,
    pub &'static str,
);
#[derive(Debug, Clone, PartialEq)]
pub struct SingleMarketMultiItemMaxBuy(
    pub [Option<(TypeId, Quantity)>; MAX_MULTI_ITEM],
    pub Market,
    pub PriceMod,
    pub &'static str,
);
#[derive(Debug, Clone, PartialEq)]
pub struct SubSingleItemsMaxBuy(
    pub [(&'static str, Quantity); MAX_SUB_ITEM],
    pub Location,
    pub &'static str,
);

trait WeveMarketMessages {
    fn to_reqs(&self) -> Vec<MarketOrdersReq>;
    fn get_price(
        &self,
        reps: Vec<(MarketOrdersReq, MarketOrdersRep)>,
    ) -> Price;
    fn price_source(&self) -> PriceSource;
}

impl PricingModel {
    pub async fn get_price(&self, client: Client) -> Result<Price, Error> {
        if let PricingModel::Rejected = self {
            return Ok(Price::Rejected);
        }
        // let reps: Vec<(MarketOrdersReq, MarketOrdersRep)> = match self {
        //     PricingModel::SingleMarketSingleItemMaxBuy(p) => p.to_reqs(),
        //     PricingModel::SingleMarketMultiItemMaxBuy(p) => p.to_reqs(),
        //     PricingModel::Rejected => return Ok(Price::Rejected),
        // }
        //     .into_iter()
        //     .map(|req| get_market_orders(client.clone(), req))
        //     .collect::<FuturesUnordered<_>>()
        //     .try_collect() // Do not try to type-annotate this
        //     .await?;
        let reps: Vec<(MarketOrdersReq, MarketOrdersRep)> = self
            .to_reqs()
            .into_iter()
            .map(|req| get_market_orders(client.clone(), req))
            .collect::<FuturesUnordered<_>>()
            .try_collect() // Do not try to type-annotate this
            .await?;
        
        // Ok(match self {
        //     PricingModel::SingleMarketSingleItemMaxBuy(p) => p.get_price(reps),
        //     PricingModel::SingleMarketMultiItemMaxBuy(p) => p.get_price(reps),
        //     PricingModel::Rejected => unreachable!(),
        // })
        Ok(self.get_price_inner(reps))
    }

    pub fn price_source(&self) -> PriceSource {
        match self {
            PricingModel::SingleMarketSingleItemMaxBuy(p) => p.price_source(),
            PricingModel::SingleMarketMultiItemMaxBuy(p) => p.price_source(),
            PricingModel::SubSingleItemsMaxBuy(p) => p.price_source(),
            PricingModel::Rejected => "Rejected".to_string(),
        }
    }

    fn to_reqs(&self) -> Vec<MarketOrdersReq> {
        match self {
            PricingModel::SingleMarketSingleItemMaxBuy(p) => p.to_reqs(),
            PricingModel::SingleMarketMultiItemMaxBuy(p) => p.to_reqs(),
            PricingModel::SubSingleItemsMaxBuy(p) => p.to_reqs(),
            PricingModel::Rejected => vec![],
        }
    }

    fn get_price_inner(
        &self,
        reps: Vec<(MarketOrdersReq, MarketOrdersRep)>,
    ) -> Price {
        match self {
            PricingModel::SingleMarketSingleItemMaxBuy(p) => p.get_price(reps),
            PricingModel::SingleMarketMultiItemMaxBuy(p) => p.get_price(reps),
            PricingModel::SubSingleItemsMaxBuy(p) => p.get_price(reps),
            PricingModel::Rejected => Price::Rejected,
        }
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

    fn price_source(&self) -> PriceSource {
        self.3.to_string()
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

    fn price_source(&self) -> PriceSource {
        self.3.to_string()
    }
}

impl SubSingleItemsMaxBuy {
    fn sub_items(&self) -> impl Iterator<Item = (&PricingModel, &'static str, Quantity)> {
        self.0
            .iter()
            .filter_map(
                |s| match *s {
                    ("", _) => None,
                    (item, qnt) => match PM_MAP
                        .get(self.1)
                        .unwrap()
                        .get(item)
                    {
                        Some(p) => Some((p, item, qnt)),
                        None => Some((&PricingModel::Rejected, item, qnt)),
                    }
                }
            )
    }
}

impl WeveMarketMessages for SubSingleItemsMaxBuy {
    fn to_reqs(&self) -> Vec<MarketOrdersReq> {
        let mut reqs: Vec<MarketOrdersReq> = Vec::with_capacity(self.0.len());
        for (pm, _, _) in self.sub_items() {
            if let PricingModel::Rejected = pm {
                return vec![];
            }
            for req in pm.to_reqs() {
                reqs.push(req);
            }
        }
        reqs
    }

    fn get_price(
        &self,
        reps: Vec<(MarketOrdersReq, MarketOrdersRep)>,
    ) -> Price {
        let mut price: f64 = 0.0;
        for (req, rep) in reps {
            for (pm, item, qnt) in self.sub_items() {
                match pm {
                    PricingModel::SingleMarketSingleItemMaxBuy(sipm) => {
                        if req.type_id == sipm.0 {
                            match sipm.get_price(vec![(req, rep)]) {
                                Price::Rejected => return Price::Rejected,
                                Price::Accepted(siprice) => price += siprice * qnt,
                            }
                            break;
                        }
                    },
                    PricingModel::Rejected => return Price::Rejected,
                    _ => panic!(
                        "{} at location {} points to invalid PricingModel",
                        item,
                        self.1,
                    ),
                }
            }
        }
        Price::Accepted(price)
    }

    fn price_source(&self) -> PriceSource {
        let mut ps = String::new();
        let mut first = true;
        for (pm, item, qnt) in self.sub_items() {
            ps.push_str(&format!(
                "{{\"item\":\"{}\",\"quantity\":{},\"description\":\"{}\"}},",
                item,
                qnt,
                pm.price_source(),
            ));
            if first {
                first = false;
            }
        }
        if !first { // remove trailing comma unless self.0 is empty
            ps.pop();
        }
        format!(
            "MP{{\"description\":\"{}\",\"values\":[{}]}}",
            self.2,
            ps,
        )
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
