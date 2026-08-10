CREATE TABLE `accounts` (
    id      BIGINT AUTO_INCREMENT PRIMARY KEY,
    name    VARCHAR(255) NOT NULL,
    user_id BIGINT NOT NULL,
    CONSTRAINT fk_accounts_user FOREIGN KEY (user_id) REFERENCES `user` (id) ON DELETE CASCADE
);
