//! Business rules. No knowledge of use cases, storage, or presentation.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    product: String,
    quantity: u32,
    total_yen: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderError {
    EmptyProduct,
    ZeroQuantity,
    TotalOverflow,
}

impl std::fmt::Display for OrderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::EmptyProduct => "product must not be empty",
            Self::ZeroQuantity => "quantity must be greater than zero",
            Self::TotalOverflow => "order total is too large",
        };
        f.write_str(message)
    }
}

impl std::error::Error for OrderError {}

impl Order {
    pub fn new(product: String, quantity: u32, unit_price_yen: u64) -> Result<Self, OrderError> {
        let product = product.trim().to_owned();
        if product.is_empty() {
            return Err(OrderError::EmptyProduct);
        }
        if quantity == 0 {
            return Err(OrderError::ZeroQuantity);
        }
        let total_yen = unit_price_yen
            .checked_mul(u64::from(quantity))
            .ok_or(OrderError::TotalOverflow)?;
        Ok(Self {
            product,
            quantity,
            total_yen,
        })
    }

    pub fn product(&self) -> &str {
        &self.product
    }

    pub fn quantity(&self) -> u32 {
        self.quantity
    }

    pub fn total_yen(&self) -> u64 {
        self.total_yen
    }
}
