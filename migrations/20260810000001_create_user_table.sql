CREATE TABLE `user` (
    id       BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
    username VARCHAR(255) NOT NULL,
    role     VARCHAR(20)  NOT NULL,
    email    VARCHAR(255) NOT NULL,
    password VARCHAR(255) NOT NULL,
    UNIQUE KEY uq_user_username (username),
    UNIQUE KEY uq_user_email (email)
);
