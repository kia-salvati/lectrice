//! The link mirrors the helper names of the core's error registry; they must stay the same names and codes.

use lectrice_domain::ErrorName;
use lectrice_link::LinkErrorName;

#[test]
fn the_links_error_names_are_the_registrys() {
    for name in LinkErrorName::ALL {
        let registry = ErrorName::from_name(name.as_str()).unwrap_or_else(|| panic!("{} is not in the registry", name.as_str()));
        assert_eq!(registry.spec().code, name.code(), "{}", name.as_str());
    }
}
