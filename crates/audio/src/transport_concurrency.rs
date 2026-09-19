use crate::Transport;

#[test]
fn seek_arriving_during_block_survives_callback_position_commit() {
    // Given
    let transport = Transport::new(32);
    transport.seek(4);
    let block_start = transport.begin_block_position();

    // When
    transport.seek(20);
    transport.commit_block_position(block_start + 8);

    // Then
    assert_eq!(transport.position(), 20);
    assert_eq!(transport.begin_block_position(), 20);
}
