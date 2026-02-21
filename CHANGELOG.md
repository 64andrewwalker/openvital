# Changelog

## [0.4.0](https://github.com/64andrewwalker/openvital/compare/v0.3.1...v0.4.0) (2026-02-20)


### Features

* **med:** add medication management feature ([2edf01c](https://github.com/64andrewwalker/openvital/commit/2edf01c4a94ddc51cd2d3f9cb4cc2ad3019a8e95))
* **med:** add Medication, Frequency, Route models with dose parsing ([6073e97](https://github.com/64andrewwalker/openvital/commit/6073e9704c24981c99532559289a4b954b9c4b01))
* **med:** add medications table migration and CRUD operations ([768eca6](https://github.com/64andrewwalker/openvital/commit/768eca6179bfc1087632c6f8a2439bd4dc1d517f))
* **med:** integrate medications with trend, goal, status, and export ([3a80b0e](https://github.com/64andrewwalker/openvital/commit/3a80b0e69294538a40fea9d01d54b3ad9ba9ffb4))


### Bug Fixes

* **med:** add overall adherence footer and migration tests ([dba744f](https://github.com/64andrewwalker/openvital/commit/dba744f165700329af3309fd46719621a0e0f56f))
* **med:** add remove confirmation and stopped take warning ([9c87001](https://github.com/64andrewwalker/openvital/commit/9c8700120423ea65b77487c182cf53ecbe205ccc))
* **med:** align adherence, correlation, and goal logic with design spec ([e8f9b0b](https://github.com/64andrewwalker/openvital/commit/e8f9b0b4ddb346b3f0d73ffba821994d823dd71a))
* **med:** align med status JSON output with design spec ([9c08b93](https://github.com/64andrewwalker/openvital/commit/9c08b938512b84cd7d6ed4aae651d0359576dd5c))
* **med:** resolve name collision, weekly adherence, status format, and list header ([562de39](https://github.com/64andrewwalker/openvital/commit/562de3900b534377c0dbe80b3b48fa7ff5b97828))

## [0.3.1](https://github.com/64andrewwalker/openvital/compare/v0.3.0...v0.3.1) (2026-02-19)


### Bug Fixes

* address 13 PM review issues (4 P0, 5 P1, 3 P2) ([6a332f8](https://github.com/64andrewwalker/openvital/commit/6a332f85dddc14a2bc898a46a616ffe540d6b321))
* address 13 PM review issues (4 P0, 5 P1, 3 P2) ([82be643](https://github.com/64andrewwalker/openvital/commit/82be6436761af8bd07bfb0577571d522a8b1f541))
* **trend:** use noun rate unit for empty results ([f0fb334](https://github.com/64andrewwalker/openvital/commit/f0fb3342652f4f8500d5fc039aea5b520638466d))

## [0.3.0](https://github.com/64andrewwalker/openvital/compare/v0.2.0...v0.3.0) (2026-02-18)


### Features

* add native imperial unit support ([#6](https://github.com/64andrewwalker/openvital/issues/6)) ([f7c092f](https://github.com/64andrewwalker/openvital/commit/f7c092fd221bc2a4393cddef3dce84a1682bdebb))


### Bug Fixes

* address 7 code review issues from imperial units PR ([2f678d5](https://github.com/64andrewwalker/openvital/commit/2f678d5a676b416f9415095aeb025112dedd431a))
* address code review issues from imperial units PR ([d726773](https://github.com/64andrewwalker/openvital/commit/d7267734bb309c2bd81bdfc8700813a40203e9f0))

## [0.2.0](https://github.com/64andrewwalker/openvital/compare/v0.1.0...v0.2.0) (2026-02-18)


### Features

* v0.2 bug fixes and UX improvements ([#3](https://github.com/64andrewwalker/openvital/issues/3)) ([736fb89](https://github.com/64andrewwalker/openvital/commit/736fb89b3a70cb6eddf496ffdf9bb910ff1ceeba))

## 0.1.0 (2026-02-18)


### Features

* add goal system, lib crate, docs, and CLAUDE.md ([d1f1921](https://github.com/64andrewwalker/openvital/commit/d1f192136204927d35ea8e3b2351324cee8b8080))
* implement report, export/import, correlation, streaks, shell completions ([c5c2120](https://github.com/64andrewwalker/openvital/commit/c5c2120a6dc21edd50ca689082bba3a716941819))
* implement trend analysis command with BDD tests ([9960067](https://github.com/64andrewwalker/openvital/commit/99600678552adba0e8822753ed37ba1832766a26))
