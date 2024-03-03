// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

//! Defines an encapsulation of "query plugins", including an interface
//! for managing and using them.

use itertools::Itertools;

use crate::package::{self, Package};
use crate::Provider;

pub use self::plugin::Plugin;
pub use self::transaction::Transaction;

pub mod plugin;
pub mod transaction;

/// A registry is composed of multiple "query plugins" that
/// provide [`Package`] information
#[derive(Debug, Default)]
pub struct Registry {
    /// Ordered set of plugins
    plugins: Vec<Plugin>,
}

impl Registry {
    /// Add a [`Plugin`] to the [`Registry`]
    pub fn add_plugin(&mut self, plugin: Plugin) {
        self.plugins.push(plugin);
    }

    fn query<'a, I>(&'a self, query: impl Fn(&'a Plugin) -> I + Copy + 'a) -> impl Iterator<Item = Package> + 'a
    where
        I: IntoIterator<Item = Package> + 'a,
    {
        self.plugins
            .iter()
            .sorted_by(|a, b| a.priority().cmp(&b.priority()).reverse())
            .flat_map(query)
    }

    /// Return a sorted stream of [`Package`] by provider
    pub fn by_provider<'a>(
        &'a self,
        provider: &'a Provider,
        flags: package::Flags,
    ) -> impl Iterator<Item = Package> + 'a {
        self.query(move |plugin| plugin.query_provider(provider, flags))
    }

    /// Return a sorted stream of [`Package`] by name
    pub fn by_name<'a>(
        &'a self,
        package_name: &'a package::Name,
        flags: package::Flags,
    ) -> impl Iterator<Item = Package> + 'a {
        self.query(move |plugin| plugin.query_name(package_name, flags))
    }

    /// Return a sorted stream of [`Package`] by id
    pub fn by_id<'a>(&'a self, id: &'a package::Id) -> impl Iterator<Item = Package> + 'a {
        self.query(move |plugin| plugin.package(id))
    }

    /// Return a sorted stream of [`Package`] matching the given [`Flags`]
    ///
    /// [`Flags`]: package::Flags
    pub fn list(&self, flags: package::Flags) -> impl Iterator<Item = Package> + '_ {
        self.query(move |plugin| plugin.list(flags))
    }

    /// Return a sorted stream of installed [`Package`]
    pub fn list_installed(&self, flags: package::Flags) -> impl Iterator<Item = Package> + '_ {
        self.list(flags | package::Flags::INSTALLED)
    }

    /// Return a sorted stream of available [`Package`]
    pub fn list_available(&self, flags: package::Flags) -> impl Iterator<Item = Package> + '_ {
        self.list(flags | package::Flags::AVAILABLE)
    }

    /// Return a new transaction for this registry
    pub fn transaction(&self) -> Result<Transaction<'_>, transaction::Error> {
        transaction::new(self)
    }

    /// Return a new transaction for this registry initialised with the incoming package set as installed
    pub fn transaction_with_installed(
        &self,
        incoming: Vec<package::Id>,
    ) -> Result<Transaction<'_>, transaction::Error> {
        transaction::new_with_installed(self, incoming)
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;

    use super::*;

    fn test_ordering() {
        let mut registry = Registry::default();

        let package = |id: &str, release| Package {
            id: package::Id::from(id.to_string()),
            meta: package::Meta {
                name: package::Name::from(id.to_string()),
                version_identifier: Default::default(),
                source_release: release,
                build_release: Default::default(),
                architecture: Default::default(),
                summary: Default::default(),
                description: Default::default(),
                source_id: Default::default(),
                homepage: Default::default(),
                licenses: Default::default(),
                dependencies: Default::default(),
                providers: Default::default(),
                uri: Default::default(),
                hash: Default::default(),
                download_size: Default::default(),
            },
            flags: package::Flags::NONE,
        };

        registry.add_plugin(Plugin::Test(plugin::Test::new(
            // Priority
            1,
            // Id / release number
            vec![package("a", 0), package("b", 100)],
        )));

        registry.add_plugin(Plugin::Test(plugin::Test::new(
            50,
            vec![package("c", 50), package("d", 1)],
        )));

        let query = registry.list(package::Flags::NONE);

        // Packages are sorted by plugin priority, desc -> release number, desc
        for (idx, package) in query.enumerate() {
            let id = |id: &str| package::Id::from(id.to_string());

            match idx {
                0 => assert_eq!(package.id, id("c")),
                1 => assert_eq!(package.id, id("d")),
                2 => assert_eq!(package.id, id("b")),
                3 => assert_eq!(package.id, id("a")),
                _ => {}
            }
        }
    }

    fn test_flags() {
        let mut registry = Registry::default();

        let package = |id: &str, flags| Package {
            id: package::Id::from(id.to_string()),
            meta: package::Meta {
                name: package::Name::from(id.to_string()),
                version_identifier: Default::default(),
                source_release: Default::default(),
                build_release: Default::default(),
                architecture: Default::default(),
                summary: Default::default(),
                description: Default::default(),
                source_id: Default::default(),
                homepage: Default::default(),
                licenses: Default::default(),
                dependencies: Default::default(),
                providers: Default::default(),
                uri: Default::default(),
                hash: Default::default(),
                download_size: Default::default(),
            },
            flags,
        };

        registry.add_plugin(Plugin::Test(plugin::test::Test::new(
            1,
            vec![
                package("a", package::Flags::INSTALLED),
                package("b", package::Flags::AVAILABLE),
                package("c", package::Flags::SOURCE),
                package("d", package::Flags::SOURCE | package::Flags::INSTALLED),
                package("e", package::Flags::SOURCE | package::Flags::AVAILABLE),
            ],
        )));

        let installed = registry.list_installed(package::Flags::NONE).collect();
        let available = registry.list_available(package::Flags::NONE).collect();
        let installed_source = registry.list_installed(package::Flags::SOURCE).collect();
        let available_source = registry.list_available(package::Flags::SOURCE).collect();

        fn matches(actual: Vec<Package>, expected: &[&'static str]) -> bool {
            let actual = actual
                .into_iter()
                .map(|p| String::from(p.meta.name))
                .collect::<HashSet<_>>();
            let expected = expected.iter().map(|s| s.to_string()).collect::<HashSet<_>>();

            actual == expected
        }

        assert!(matches(installed, &["a", "d"]));
        assert!(matches(available, &["b", "e"]));
        assert!(matches(installed_source, &["d"]));
        assert!(matches(available_source, &["e"]));
    }
}