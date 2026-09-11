use clean_architecture_example::application::{
    OrderRepository, PlaceOrderError, PlaceOrderInput, RepositoryError, place_order,
};
use clean_architecture_example::domain::{Order, OrderError};
use clean_architecture_example::infrastructure::InMemoryOrderRepository;
use clean_architecture_example::presentation::submit_order;

fn input(quantity: u32) -> PlaceOrderInput {
    PlaceOrderInput {
        product: "Coffee".to_owned(),
        quantity,
        unit_price_yen: 600,
    }
}

#[test]
fn calculates_total_and_normalizes_product() {
    let order = Order::new(" Coffee ".to_owned(), 2, 600).unwrap();
    assert_eq!(order.product(), "Coffee");
    assert_eq!(order.quantity(), 2);
    assert_eq!(order.total_yen(), 1200);
}

#[test]
fn rejects_empty_product() {
    assert_eq!(
        Order::new("  ".to_owned(), 1, 600),
        Err(OrderError::EmptyProduct)
    );
}

#[test]
fn rejects_zero_quantity() {
    assert_eq!(
        Order::new("Coffee".to_owned(), 0, 600),
        Err(OrderError::ZeroQuantity)
    );
}

#[test]
fn rejects_total_overflow() {
    assert_eq!(
        Order::new("Coffee".to_owned(), 2, u64::MAX),
        Err(OrderError::TotalOverflow)
    );
}

#[test]
fn saves_orders_with_distinct_ids() {
    let mut repository = InMemoryOrderRepository::default();
    let first = place_order(&mut repository, input(2)).unwrap();
    let second = place_order(&mut repository, input(1)).unwrap();
    assert_eq!((first.order_id, second.order_id), (1, 2));
    assert_eq!(first.total_yen, 1200);
    assert_eq!(repository.orders().len(), 2);
}

#[test]
fn invalid_input_does_not_reach_storage() {
    let mut repository = InMemoryOrderRepository::default();
    assert_eq!(
        place_order(&mut repository, input(0)),
        Err(PlaceOrderError::InvalidOrder(OrderError::ZeroQuantity))
    );
    assert!(repository.orders().is_empty());
}

#[test]
fn propagates_failure_through_the_repository_port() {
    struct FailingRepository;
    impl OrderRepository for FailingRepository {
        fn save(&mut self, _: &Order) -> Result<u64, RepositoryError> {
            Err(RepositoryError)
        }
    }
    assert_eq!(
        place_order(&mut FailingRepository, input(1)),
        Err(PlaceOrderError::Persistence(RepositoryError))
    );
}

#[test]
fn presentation_uses_the_use_case() {
    let mut repository = InMemoryOrderRepository::default();
    assert_eq!(
        submit_order(&mut repository, "Coffee", 2, 600).unwrap(),
        "order #1: 1200 JPY"
    );
}
