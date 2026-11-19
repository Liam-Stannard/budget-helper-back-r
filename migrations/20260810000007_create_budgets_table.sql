-- One monthly spending limit per (user, category). `category` uses the same VARCHAR encoding as
-- `transactions.category` (see `TransactionCategory`); `INCOME` is rejected at the API layer,
-- since a spending limit on income is meaningless.
CREATE TABLE `budgets` (
    id            BIGINT AUTO_INCREMENT PRIMARY KEY,
    user_id       BIGINT         NOT NULL,
    category      VARCHAR(20)    NOT NULL,
    monthly_limit DECIMAL(19, 4) NOT NULL,
    UNIQUE KEY uq_budgets_user_category (user_id, category),
    CONSTRAINT fk_budgets_user FOREIGN KEY (user_id) REFERENCES `user` (id) ON DELETE CASCADE
);
