use std::cell::RefCell;

use super::*;

#[derive(Default)]
struct FakeNpm {
    packages: RefCell<BTreeMap<String, String>>,
    installations: RefCell<Vec<String>>,
    offline: bool,
}

impl PackageManager for FakeNpm {
    fn installed_version(&self, package: &str) -> Result<Option<String>> {
        Ok(self.packages.borrow().get(package).cloned())
    }

    fn install(&self, package: &str, version: &str) -> Result<()> {
        if self.offline {
            return Err("registry unavailable".into());
        }
        self.installations.borrow_mut().push(package.into());
        self.packages
            .borrow_mut()
            .insert(package.into(), version.into());
        Ok(())
    }
}

fn pinned_packages() -> BTreeMap<String, String> {
    serde_json::from_str::<Manifest>(include_str!("../managed-server/package.json"))
        .unwrap()
        .dependencies
}

#[test]
fn installs_the_complete_pinned_stack() {
    let npm = FakeNpm::default();
    ensure_installed(&npm).unwrap();
    assert_eq!(*npm.packages.borrow(), pinned_packages());
    assert_eq!(npm.installations.borrow().len(), 3);

    let packages = npm.packages.borrow();
    assert_eq!(
        packages["@angular/language-server"],
        packages["@angular/language-service"]
    );
    for version in packages.values() {
        let parts: Vec<_> = version.split('.').collect();
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|part| part.parse::<u32>().is_ok()));
    }
}

#[test]
fn a_new_session_reuses_the_complete_stack_offline() {
    let npm = FakeNpm {
        packages: RefCell::new(pinned_packages()),
        offline: true,
        ..Default::default()
    };
    ensure_installed(&npm).unwrap();
    ensure_installed(&npm).unwrap();
    assert!(npm.installations.borrow().is_empty());
}

#[test]
fn repairs_missing_and_different_versions_without_updating_matching_packages() {
    let npm = FakeNpm {
        packages: RefCell::new(pinned_packages()),
        ..Default::default()
    };
    npm.packages.borrow_mut().remove("typescript");
    npm.packages
        .borrow_mut()
        .insert("@angular/language-service".into(), "99.0.0".into());

    ensure_installed(&npm).unwrap();
    assert_eq!(*npm.packages.borrow(), pinned_packages());
    assert_eq!(
        *npm.installations.borrow(),
        ["@angular/language-service", "typescript"]
    );
}

#[test]
fn incomplete_offline_installation_fails_and_can_be_retried() {
    let mut npm = FakeNpm {
        packages: RefCell::new(pinned_packages()),
        offline: true,
        ..Default::default()
    };
    npm.packages.borrow_mut().remove("typescript");

    let error = ensure_installed(&npm).unwrap_err();
    assert!(error.contains("typescript@"));
    assert!(error.contains("registry unavailable"));

    npm.offline = false;
    ensure_installed(&npm).unwrap();
    assert_eq!(*npm.packages.borrow(), pinned_packages());
}
