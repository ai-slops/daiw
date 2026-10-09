use crate::definition_composition::COMPOSITION_WORK_LIMIT;
use crate::error::CoreError;

pub fn reserve<T>(items: &mut Vec<T>, added: usize) -> Result<(), CoreError> {
    let final_count = items
        .len()
        .checked_add(added)
        .ok_or_else(|| CoreError::InvalidProject("pattern note count overflow".into()))?;
    let limit = usize::try_from(COMPOSITION_WORK_LIMIT)
        .map_err(|_| CoreError::InvalidProject("composition limit exceeds platform".into()))?;
    if final_count > limit {
        return Err(CoreError::InvalidProject(format!(
            "pattern note limit exceeded: {final_count} > {limit}"
        )));
    }
    items
        .try_reserve(added)
        .map_err(|_| CoreError::InvalidProject("pattern note allocation failed".into()))
}

#[cfg(test)]
mod tests {
    use super::reserve;

    #[test]
    fn operation_growth_is_bounded_at_composition_limit() {
        // Given
        let mut accepted = Vec::<()>::new();
        let mut rejected = Vec::<()>::new();

        // When
        let within = reserve(&mut accepted, 100_000);
        let beyond = reserve(&mut rejected, 100_001);

        // Then
        assert!(within.is_ok());
        assert!(beyond.is_err());
    }
}
