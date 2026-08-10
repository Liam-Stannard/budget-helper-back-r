CREATE TABLE `transactions` (
    id         BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
    category   VARCHAR(20)    NOT NULL,
    title      VARCHAR(255)   NOT NULL,
    amount     DECIMAL(19, 4) NOT NULL,
    date       DATE           NOT NULL,
    account_id BIGINT UNSIGNED NOT NULL,
    user_id    BIGINT UNSIGNED NOT NULL,
    CONSTRAINT fk_transactions_account FOREIGN KEY (account_id) REFERENCES `accounts` (id) ON DELETE CASCADE,
    CONSTRAINT fk_transactions_user FOREIGN KEY (user_id) REFERENCES `user` (id) ON DELETE CASCADE
);
