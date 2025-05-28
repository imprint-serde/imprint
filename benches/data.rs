use fake::faker::company::en::*;
use fake::faker::lorem::en::*;
use fake::{Fake, Faker};
use imprint::{ImprintError, ImprintRecord, ImprintWriter, SchemaId, Value};

pub struct Product {
    id: String,
    name: String,
    description: String,
    price: f64,
    quantity: i32,
    category: String,
    brand: String,
    tags: Vec<String>,
    sku: String,
}

impl Product {
    pub fn fake(size: usize) -> Self {
        Self {
            id: Faker.fake::<String>(),
            name: Words(size..(size * 2)).fake::<Vec<String>>().join(" "),
            description: Paragraph(size..(size * 2)).fake(),
            price: (10.0..1000.0).fake(),
            quantity: (0..1000).fake(),
            category: Words(1..2).fake::<Vec<String>>().join(" "),
            brand: CompanyName().fake(),
            tags: Words(size * 2..size * 3).fake::<Vec<String>>(),
            sku: Faker.fake::<String>(),
        }
    }

    pub fn to_imprint(&self) -> Result<ImprintRecord, ImprintError> {
        let mut writer = ImprintWriter::new(SchemaId {
            fieldspace_id: 0,
            schema_hash: 0,
        })
        .unwrap();

        writer.add_field(1, Value::String(self.id.clone()))?;
        writer.add_field(2, Value::String(self.name.clone()))?;
        writer.add_field(3, Value::String(self.description.clone()))?;
        writer.add_field(4, Value::Float64(self.price))?;
        writer.add_field(5, Value::Int32(self.quantity))?;
        writer.add_field(6, Value::String(self.category.clone()))?;
        writer.add_field(7, Value::String(self.brand.clone()))?;
        writer.add_field(
            8,
            Value::Array(self.tags.iter().map(|t| Value::String(t.clone())).collect()),
        )?;
        writer.add_field(9, Value::String(self.sku.clone()))?;

        Ok(writer.build()?)
    }
}

pub struct Order {
    id: String,
    product_id: String,
    customer_id: String,
    quantity: i32,
    tags: Vec<String>,
}

impl Order {
    pub fn fake(size: usize) -> Self {
        Self {
            id: Faker.fake::<String>(),
            product_id: Faker.fake::<String>(),
            customer_id: Faker.fake::<String>(),
            quantity: (0..1000).fake(),
            tags: Words(size..size * 2).fake::<Vec<String>>(),
        }
    }

    pub fn to_imprint(&self) -> Result<ImprintRecord, ImprintError> {
        let mut writer = ImprintWriter::new(SchemaId {
            fieldspace_id: 0,
            schema_hash: 1,
        })?;

        writer.add_field(101, Value::String(self.id.clone()))?;
        writer.add_field(102, Value::String(self.customer_id.clone()))?;
        writer.add_field(103, Value::String(self.product_id.clone()))?;
        writer.add_field(104, Value::Int32(self.quantity))?;
        writer.add_field(
            105,
            Value::Array(self.tags.iter().map(|t| Value::String(t.clone())).collect()),
        )?;

        Ok(writer.build()?)
    }
}
