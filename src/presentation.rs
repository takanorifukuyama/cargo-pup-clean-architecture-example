//! A framework-free input adapter. It never chooses a concrete repository.

use crate::application::OrderRepository;
use crate::application::PlaceOrderError;
use crate::application::PlaceOrderInput;
use crate::application::place_order;

pub fn submit_order<R: OrderRepository>(
    repository: &mut R,
    product: &str,
    quantity: u32,
    unit_price_yen: u64,
) -> Result<String, PlaceOrderError> {
    let receipt = place_order(
        repository,
        PlaceOrderInput {
            product: product.to_owned(),
            quantity,
            unit_price_yen,
        },
    )?;
    Ok(format!(
        "order #{}: {} JPY",
        receipt.order_id, receipt.total_yen
    ))
}
