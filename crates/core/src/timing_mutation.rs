use crate::{CoreError, MeterMap, Project, TempoMap};

pub fn set_tempo(project: &mut Project, tempo: f64) -> Result<String, CoreError> {
    project.tempo = tempo;
    project.tempo_map = None;
    Ok("Set tempo".to_owned())
}

pub fn set_tempo_map(project: &mut Project, tempo_map: TempoMap) -> Result<String, CoreError> {
    let first = tempo_map
        .points()
        .first()
        .ok_or_else(|| CoreError::InvalidParams("tempo map must contain a point".to_owned()))?;
    project.tempo = first.bpm;
    project.tempo_map = Some(tempo_map);
    Ok("Set tempo map".to_owned())
}

pub fn set_meter(project: &mut Project, meter: [u8; 2]) -> Result<String, CoreError> {
    project.meter = meter;
    project.meter_map = None;
    Ok("Set meter".to_owned())
}

pub fn set_meter_map(project: &mut Project, meter_map: MeterMap) -> Result<String, CoreError> {
    let first = meter_map
        .points()
        .first()
        .ok_or_else(|| CoreError::InvalidParams("meter map must contain a point".to_owned()))?;
    project.meter = [first.numerator, first.denominator];
    project.meter_map = Some(meter_map);
    Ok("Set meter map".to_owned())
}
