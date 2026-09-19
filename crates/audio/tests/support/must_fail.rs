pub fn must_fail<T, E>(result: Result<T, E>) -> E {
    match result {
        Ok(_) => panic!("expected operation to fail"),
        Err(error) => error,
    }
}
