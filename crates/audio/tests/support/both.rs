pub fn must<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("unexpected error: {error:?}"),
    }
}

pub fn must_fail<T, E>(result: Result<T, E>) -> E {
    match result {
        Ok(_) => panic!("expected operation to fail"),
        Err(error) => error,
    }
}
