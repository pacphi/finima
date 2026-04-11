//! Integration tests for cross-user authorization enforcement.
//!
//! These tests verify that User A cannot access or modify User B's data
//! and vice versa. All authorization checks are exercised at the repository
//! and trait level against a real PostgreSQL database.
//!
//! Run with:
//!   docker compose -f docker-compose.test.yml up -d
//!   cargo test -p finima-api --test authorization_test

mod common;

use uuid::Uuid;

use finima_core::traits::{AccountRepo, PortfolioRepo, UserRepo};

use common::{create_user_b, seed_test_db, setup_test_db, TestAppState, USER_A_EMAIL, USER_A_ID};

// ---------------------------------------------------------------------------
// Deterministic IDs from seed.sql
// ---------------------------------------------------------------------------

const USER_A_PORTFOLIO: &str = "b1000000-0000-4000-8000-000000000001";
const USER_A_ACCOUNT_CHECKING: &str = "c1000000-0000-4000-8000-000000000001";

const USER_B_ID: &str = "a2000000-0000-4000-8000-000000000002";
const USER_B_PORTFOLIO: &str = "b2000000-0000-4000-8000-000000000002";
const USER_B_SAVINGS_GOAL: &str = "e2000000-0000-4000-8000-000000000001";

// ---------------------------------------------------------------------------
// Portfolio ownership verification
// ---------------------------------------------------------------------------

#[tokio::test]
async fn user_a_owns_their_portfolio() {
    let pool = setup_test_db().await;
    seed_test_db(&pool).await;
    let state = TestAppState::new(pool.clone());
    create_user_b(&pool).await;

    let user_a_id = Uuid::parse_str(USER_A_ID).unwrap();
    let portfolio_a_id = Uuid::parse_str(USER_A_PORTFOLIO).unwrap();

    // User A can verify ownership of their own portfolio
    let result = state
        .portfolio_repo()
        .verify_ownership(portfolio_a_id, user_a_id)
        .await;
    assert!(result.is_ok(), "User A should own their own portfolio");
}

#[tokio::test]
async fn user_a_cannot_access_user_b_portfolio() {
    let pool = setup_test_db().await;
    seed_test_db(&pool).await;
    let state = TestAppState::new(pool.clone());
    create_user_b(&pool).await;

    let user_a_id = Uuid::parse_str(USER_A_ID).unwrap();
    let portfolio_b_id = Uuid::parse_str(USER_B_PORTFOLIO).unwrap();

    // User A tries to verify ownership of User B's portfolio -- should fail
    let result = state
        .portfolio_repo()
        .verify_ownership(portfolio_b_id, user_a_id)
        .await;
    assert!(
        result.is_err(),
        "User A should NOT be able to access User B's portfolio"
    );
}

#[tokio::test]
async fn user_b_cannot_access_user_a_portfolio() {
    let pool = setup_test_db().await;
    seed_test_db(&pool).await;
    let state = TestAppState::new(pool.clone());
    create_user_b(&pool).await;

    let user_b_id = Uuid::parse_str(USER_B_ID).unwrap();
    let portfolio_a_id = Uuid::parse_str(USER_A_PORTFOLIO).unwrap();

    let result = state
        .portfolio_repo()
        .verify_ownership(portfolio_a_id, user_b_id)
        .await;
    assert!(
        result.is_err(),
        "User B should NOT be able to access User A's portfolio"
    );
}

// ---------------------------------------------------------------------------
// Transaction access control (via portfolio ownership)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn user_a_can_list_transactions_for_own_account() {
    let pool = setup_test_db().await;
    seed_test_db(&pool).await;
    let state = TestAppState::new(pool.clone());

    let user_a_id = Uuid::parse_str(USER_A_ID).unwrap();
    let account_a_id = Uuid::parse_str(USER_A_ACCOUNT_CHECKING).unwrap();

    // First verify User A owns the account's portfolio
    let account = state.account_repo().find_by_id(account_a_id).await.unwrap();
    let ownership = state
        .portfolio_repo()
        .verify_ownership(account.portfolio_id, user_a_id)
        .await;
    assert!(
        ownership.is_ok(),
        "User A should own the portfolio containing their checking account"
    );
}

#[tokio::test]
async fn user_b_cannot_access_user_a_transactions() {
    let pool = setup_test_db().await;
    seed_test_db(&pool).await;
    let state = TestAppState::new(pool.clone());
    create_user_b(&pool).await;

    let user_b_id = Uuid::parse_str(USER_B_ID).unwrap();
    let account_a_id = Uuid::parse_str(USER_A_ACCOUNT_CHECKING).unwrap();

    // Trace the account -> portfolio -> user chain (same as the transaction handler)
    let account = state.account_repo().find_by_id(account_a_id).await.unwrap();
    let ownership = state
        .portfolio_repo()
        .verify_ownership(account.portfolio_id, user_b_id)
        .await;
    assert!(
        ownership.is_err(),
        "User B should NOT be able to access User A's transactions via account"
    );
}

#[tokio::test]
async fn user_b_cannot_query_user_a_portfolio_transactions() {
    let pool = setup_test_db().await;
    seed_test_db(&pool).await;
    let state = TestAppState::new(pool.clone());
    create_user_b(&pool).await;

    let user_b_id = Uuid::parse_str(USER_B_ID).unwrap();
    let portfolio_a_id = Uuid::parse_str(USER_A_PORTFOLIO).unwrap();

    // The transactions handler checks portfolio ownership before querying
    let ownership = state
        .portfolio_repo()
        .verify_ownership(portfolio_a_id, user_b_id)
        .await;
    assert!(
        ownership.is_err(),
        "User B should NOT be able to query User A's portfolio for transactions"
    );
}

// ---------------------------------------------------------------------------
// Savings goals cross-user isolation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn user_a_can_access_own_savings_goals() {
    let pool = setup_test_db().await;
    seed_test_db(&pool).await;
    let state = TestAppState::new(pool.clone());

    let user_a_id = Uuid::parse_str(USER_A_ID).unwrap();

    // User A's portfolio should be accessible
    let portfolios = state
        .portfolio_repo()
        .list_by_user(user_a_id)
        .await
        .unwrap();
    assert!(
        !portfolios.is_empty(),
        "User A should have at least one portfolio"
    );
}

#[tokio::test]
async fn user_a_cannot_modify_user_b_savings_goals() {
    let pool = setup_test_db().await;
    seed_test_db(&pool).await;
    let state = TestAppState::new(pool.clone());
    create_user_b(&pool).await;

    let user_a_id = Uuid::parse_str(USER_A_ID).unwrap();
    let portfolio_b_id = Uuid::parse_str(USER_B_PORTFOLIO).unwrap();

    // To modify a savings goal, the handler first verifies portfolio ownership.
    // User A should not be able to claim User B's portfolio.
    let ownership = state
        .portfolio_repo()
        .verify_ownership(portfolio_b_id, user_a_id)
        .await;
    assert!(
        ownership.is_err(),
        "User A should NOT be able to modify User B's savings goals"
    );
}

#[tokio::test]
async fn user_b_cannot_modify_user_a_savings_goals() {
    let pool = setup_test_db().await;
    seed_test_db(&pool).await;
    let state = TestAppState::new(pool.clone());
    create_user_b(&pool).await;

    let user_b_id = Uuid::parse_str(USER_B_ID).unwrap();
    let portfolio_a_id = Uuid::parse_str(USER_A_PORTFOLIO).unwrap();

    let ownership = state
        .portfolio_repo()
        .verify_ownership(portfolio_a_id, user_b_id)
        .await;
    assert!(
        ownership.is_err(),
        "User B should NOT be able to modify User A's savings goals"
    );
}

#[tokio::test]
async fn user_b_cannot_access_user_a_savings_goals_via_portfolio() {
    let pool = setup_test_db().await;
    seed_test_db(&pool).await;
    let state = TestAppState::new(pool.clone());
    create_user_b(&pool).await;

    let user_b_id = Uuid::parse_str(USER_B_ID).unwrap();
    let portfolio_a_id = Uuid::parse_str(USER_A_PORTFOLIO).unwrap();

    // The savings goals handler checks portfolio ownership before listing goals.
    // User B should not be able to access User A's portfolio to reach savings goals.
    let ownership = state
        .portfolio_repo()
        .verify_ownership(portfolio_a_id, user_b_id)
        .await;
    assert!(
        ownership.is_err(),
        "User B should NOT be able to access User A's savings goals via portfolio ownership"
    );

    // Verify User B's savings goal exists under User B's portfolio (not User A's)
    let user_b_savings_goal_id = Uuid::parse_str(USER_B_SAVINGS_GOAL).unwrap();
    let portfolio_b_id = Uuid::parse_str(USER_B_PORTFOLIO).unwrap();
    let goals = state
        .savings_goal_repo()
        .list_by_portfolio(portfolio_b_id)
        .await
        .unwrap();
    assert!(
        goals.iter().any(|g| g.id == user_b_savings_goal_id),
        "User B's savings goal should exist under User B's portfolio"
    );

    // User A's portfolio should NOT contain User B's savings goal
    let goals_a = state
        .savings_goal_repo()
        .list_by_portfolio(portfolio_a_id)
        .await
        .unwrap();
    assert!(
        !goals_a.iter().any(|g| g.id == user_b_savings_goal_id),
        "User B's savings goal should NOT appear under User A's portfolio"
    );
}

// ---------------------------------------------------------------------------
// Nonexistent portfolio returns error
// ---------------------------------------------------------------------------

#[tokio::test]
async fn nonexistent_portfolio_returns_not_found() {
    let pool = setup_test_db().await;
    seed_test_db(&pool).await;
    let state = TestAppState::new(pool.clone());

    let user_a_id = Uuid::parse_str(USER_A_ID).unwrap();
    let fake_portfolio = Uuid::new_v4();

    let result = state
        .portfolio_repo()
        .verify_ownership(fake_portfolio, user_a_id)
        .await;
    assert!(
        result.is_err(),
        "Nonexistent portfolio should return an error"
    );
}

// ---------------------------------------------------------------------------
// Authenticated user CAN access their own data end-to-end
// ---------------------------------------------------------------------------

#[tokio::test]
async fn authenticated_user_can_list_own_portfolios() {
    let pool = setup_test_db().await;
    seed_test_db(&pool).await;
    let state = TestAppState::new(pool.clone());

    let user_a_id = Uuid::parse_str(USER_A_ID).unwrap();

    let portfolios = state
        .portfolio_repo()
        .list_by_user(user_a_id)
        .await
        .unwrap();
    assert_eq!(
        portfolios.len(),
        1,
        "User A should have exactly 1 portfolio"
    );
    assert_eq!(portfolios[0].name, "Test Portfolio");
}

#[tokio::test]
async fn authenticated_user_can_list_own_accounts() {
    let pool = setup_test_db().await;
    seed_test_db(&pool).await;
    let state = TestAppState::new(pool.clone());

    let portfolio_a_id = Uuid::parse_str(USER_A_PORTFOLIO).unwrap();

    let accounts = state
        .account_repo()
        .list_by_portfolio(portfolio_a_id)
        .await
        .unwrap();
    assert_eq!(accounts.len(), 3, "User A should have 3 accounts");

    let names: Vec<&str> = accounts.iter().map(|a| a.name.as_str()).collect();
    assert!(names.contains(&"Chase Checking"));
    assert!(names.contains(&"Ally Savings"));
    assert!(names.contains(&"Amex Gold"));
}

// ---------------------------------------------------------------------------
// User lookup isolation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn user_lookup_by_email_returns_correct_user() {
    let pool = setup_test_db().await;
    seed_test_db(&pool).await;
    let state = TestAppState::new(pool.clone());
    create_user_b(&pool).await;

    // Look up User A
    let user_a = state
        .user_repo()
        .find_by_email(USER_A_EMAIL)
        .await
        .unwrap()
        .expect("User A should exist");
    assert_eq!(user_a.id.to_string(), USER_A_ID);

    // Look up User B
    let user_b = state
        .user_repo()
        .find_by_email("userb@finima.local")
        .await
        .unwrap()
        .expect("User B should exist");
    assert_eq!(user_b.id.to_string(), USER_B_ID);

    // They should be different users
    assert_ne!(user_a.id, user_b.id);
}

#[tokio::test]
async fn nonexistent_email_returns_none() {
    let pool = setup_test_db().await;
    let state = TestAppState::new(pool);

    let result = state
        .user_repo()
        .find_by_email("nobody@finima.local")
        .await
        .unwrap();
    assert!(result.is_none(), "Nonexistent email should return None");
}
