mod cdj;
mod flx;

pub fn builtin_profiles() -> Vec<crate::MidiProfile> {
    let mut profiles = cdj::profiles();
    profiles.extend(flx::profiles());
    profiles
}
