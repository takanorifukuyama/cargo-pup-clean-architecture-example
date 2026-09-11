//! In-memory adapter implementing the application-owned output port.

use crate::application::OrderRepository;
use crate::application::RepositoryError;
use crate::domain::Order;

#[derive(Default)]
pub struct InMemoryOrderRepository {
    orders: Vec<Order>,
    last_id: u64,
}

impl InMemoryOrderRepository {
    pub fn orders(&self) -> &[Order] {
        &self.orders
    }
}

impl OrderRepository for InMemoryOrderRepository {
    fn save(&mut self, order: &Order) -> Result<u64, RepositoryError> {
        let id = self.last_id.checked_add(1).ok_or(RepositoryError)?;
        self.orders.push(order.clone());
        self.last_id = id;
        Ok(id)
    }
}
