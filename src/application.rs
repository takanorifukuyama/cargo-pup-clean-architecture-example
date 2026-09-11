//! Use case and output port. The interface belongs to the inner layer.

use crate::domain::Order;
use crate::domain::OrderError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepositoryError;

impl std::fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("order could not be saved")
    }
}

impl std::error::Error for RepositoryError {}

pub trait OrderRepository {
    fn save(&mut self, order: &Order) -> Result<u64, RepositoryError>;
}

pub struct PlaceOrderInput {
    pub product: String,
    pub quantity: u32,
    pub unit_price_yen: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Receipt {
    pub order_id: u64,
    pub total_yen: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PlaceOrderError {
    InvalidOrder(OrderError),
    Persistence(RepositoryError),
}

impl std::fmt::Display for PlaceOrderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidOrder(error) => write!(f, "invalid order: {error}"),
            Self::Persistence(error) => write!(f, "persistence failed: {error}"),
        }
    }
}

impl std::error::Error for PlaceOrderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidOrder(error) => Some(error),
            Self::Persistence(error) => Some(error),
        }
    }
}

pub fn place_order<R: OrderRepository>(
    repository: &mut R,
    input: PlaceOrderInput,
) -> Result<Receipt, PlaceOrderError> {
    let order = Order::new(input.product, input.quantity, input.unit_price_yen)
        .map_err(PlaceOrderError::InvalidOrder)?;
    let order_id = repository.save(&order).map_err(PlaceOrderError::Persistence)?;
    Ok(Receipt {
        order_id,
        total_yen: order.total_yen(),
    })
}
