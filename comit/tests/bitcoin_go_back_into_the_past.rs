pub mod bitcoin_helper;

use bitcoin::Address;
use bitcoin_helper::BitcoinConnectorMock;
use chrono::NaiveDateTime;
use comit::btsieve::bitcoin::{matching_transaction, TransactionPattern};
use std::str::FromStr;

#[tokio::test]
async fn find_transaction_go_back_into_the_past() {
    let block1_with_transaction: bitcoin::Block = include_hex!(
        "./test_data/bitcoin/find_transaction_go_back_into_the_past/block1_with_transaction.hex"
    );
    let connector = BitcoinConnectorMock::new(
        vec![
            include_hex!("./test_data/bitcoin/find_transaction_go_back_into_the_past/block3.hex"),
            include_hex!("./test_data/bitcoin/find_transaction_go_back_into_the_past/block4.hex"),
            include_hex!("./test_data/bitcoin/find_transaction_go_back_into_the_past/block5.hex"),
        ],
        vec![
            block1_with_transaction.clone(),
            include_hex!("./test_data/bitcoin/find_transaction_go_back_into_the_past/block2.hex"),
            include_hex!("./test_data/bitcoin/find_transaction_go_back_into_the_past/block3.hex"),
            include_hex!("./test_data/bitcoin/find_transaction_go_back_into_the_past/block4.hex"),
            include_hex!("./test_data/bitcoin/find_transaction_go_back_into_the_past/block5.hex"),
        ],
    );

    let pattern = TransactionPattern {
        to_address: Some(
            Address::from_str(
                include_str!("test_data/bitcoin/find_transaction_go_back_into_the_past/address")
                    .trim(),
            )
            .unwrap(),
        ),
        from_outpoint: None,
        unlock_script: None,
    };
    let start_of_swap =
        NaiveDateTime::from_timestamp(block1_with_transaction.header.time as i64, 0);
    let expected_transaction = matching_transaction(connector, pattern, start_of_swap)
        .await
        .unwrap();

    assert_eq!(
        expected_transaction,
        include_hex!("./test_data/bitcoin/find_transaction_go_back_into_the_past/transaction.hex")
    );
}
