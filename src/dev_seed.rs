//! Opt-in local dev convenience: populates the database with a demo user, a
//! few accounts and some transactions. Gated behind `seed_dev_data` and
//! skipped entirely if the demo user already exists, so it's safe to leave
//! on across restarts.
use chrono::{Datelike, Duration, Months, Utc};
use rust_decimal_macros::dec;
use sqlx::MySqlPool;

use crate::domain::account;
use crate::domain::budget;
use crate::domain::transaction::{
    self,
    model::{Transaction, TransactionCategory},
};
use crate::domain::user::{
    self,
    model::{Role, User},
    password,
};
use crate::error::AppError;

const DEMO_USERNAME: &str = "demo";
const DEMO_PASSWORD: &str = "password123";
const DEMO_EMAIL: &str = "demo@example.com";

pub async fn run(pool: &MySqlPool) -> Result<(), AppError> {
    let already_seeded: Option<i64> = sqlx::query_scalar("SELECT id FROM user WHERE username = ?")
        .bind(DEMO_USERNAME)
        .fetch_optional(pool)
        .await?;

    if already_seeded.is_some() {
        tracing::info!("dev seed data already present, skipping");
        return Ok(());
    }

    let demo_user = user::repository::create(
        pool,
        &User {
            id: 0,
            username: DEMO_USERNAME.to_string(),
            role: Role::RoleUser,
            email: DEMO_EMAIL.to_string(),
            password: password::hash_password(DEMO_PASSWORD)?,
        },
    )
    .await?;

    let checking =
        account::repository::create(pool, "Checking", dec!(2000.00), demo_user.id).await?;
    let savings =
        account::repository::create(pool, "Savings", dec!(5000.00), demo_user.id).await?;
    let credit_card =
        account::repository::create(pool, "Credit Card", dec!(0.00), demo_user.id).await?;

    let today = Utc::now().date_naive();

    let seed_transactions: &[(i64, TransactionCategory, &str, rust_decimal::Decimal, i64)] = &[
        (checking.id, TransactionCategory::Bill, "Electricity bill", dec!(84.32), 2),
        (checking.id, TransactionCategory::Bill, "Internet bill", dec!(59.99), 5),
        (checking.id, TransactionCategory::Food, "Grocery run", dec!(112.47), 3),
        (checking.id, TransactionCategory::Food, "Farmers market", dec!(38.20), 10),
        (checking.id, TransactionCategory::Subscription, "Music streaming", dec!(11.99), 1),
        (savings.id, TransactionCategory::Bill, "Transfer to savings", dec!(250.00), 14),
        (credit_card.id, TransactionCategory::Food, "Takeout dinner", dec!(27.85), 4),
        (credit_card.id, TransactionCategory::Clothing, "New jacket", dec!(89.99), 20),
        (credit_card.id, TransactionCategory::Clothing, "Running shoes", dec!(64.50), 33),
        (credit_card.id, TransactionCategory::Subscription, "Video streaming", dec!(15.49), 6),
        (credit_card.id, TransactionCategory::Food, "Coffee shop", dec!(6.75), 1),
        (checking.id, TransactionCategory::Bill, "Phone bill", dec!(45.00), 8),
    ];

    for (account_id, category, title, amount, days_ago) in seed_transactions {
        transaction::repository::create(
            pool,
            &Transaction {
                id: 0,
                category: *category,
                title: title.to_string(),
                amount: *amount,
                date: today - Duration::days(*days_ago),
                account_id: *account_id,
                user_id: demo_user.id,
            },
        )
        .await?;
    }

    // Salary is anchored to the 1st of this month and of last month (rather than a fixed
    // `days_ago` like the rows above) so both months always have income, whatever day the seed
    // happens to run on — the Home screen's income/net cards compare the two.
    let this_month_start = today.with_day(1).expect("day 1 always exists");
    let last_month_start = this_month_start - Months::new(1);
    for date in [last_month_start, this_month_start] {
        transaction::repository::create(
            pool,
            &Transaction {
                id: 0,
                category: TransactionCategory::Income,
                title: "Salary".to_string(),
                amount: dec!(3200.00),
                date,
                account_id: checking.id,
                user_id: demo_user.id,
            },
        )
        .await?;
    }

    let seed_budgets = [
        (TransactionCategory::Food, dec!(250.00)),
        (TransactionCategory::Bill, dec!(500.00)),
        (TransactionCategory::Subscription, dec!(40.00)),
        (TransactionCategory::Clothing, dec!(100.00)),
    ];
    for (category, monthly_limit) in seed_budgets {
        budget::repository::upsert(pool, demo_user.id, category, monthly_limit).await?;
    }

    tracing::info!(
        "seeded dev data: user '{}' / '{}' with {} accounts, {} transactions and {} budgets",
        DEMO_USERNAME,
        DEMO_PASSWORD,
        3,
        seed_transactions.len() + 2,
        seed_budgets.len()
    );

    Ok(())
}
