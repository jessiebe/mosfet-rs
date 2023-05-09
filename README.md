<a href="https://opensource.newrelic.com/oss-category/#community-project"><picture><source media="(prefers-color-scheme: dark)" srcset="https://github.com/newrelic/opensource-website/raw/main/src/images/categories/dark/Community_Project.png"><source media="(prefers-color-scheme: light)" srcset="https://github.com/newrelic/opensource-website/raw/main/src/images/categories/Community_Project.png"><img alt="New Relic Open Source community project banner." src="https://github.com/newrelic/opensource-website/raw/main/src/images/categories/Community_Project.png"></picture></a>

# OpAMP Rust

> The Open Agent Management Protocol (OpAMP) is a network protocol for remote management of agents. This project aims to implement the OpAMP protocol in a client library that can be used by Supervisor processes. By leveraging Rust's memory safety and performance, we aim to provide a reliable and efficient way of transmitting observability metrics.

## Getting Started

> This library is published as a standard crate. To add it to your project, include it as a dependency inside Cargo.toml like so:
```
[dependencies]
otel-opamp-rs = "*"
```
## Features
> This library supports the following capabilities
 - HTTP support
 - Websocket support
 - Gzip compression
 - Low resource consumption

The code references stable releases of the OpAMP protocol protobuf definition [here](https://github.com/open-telemetry/opamp-spec) and aims to be standards compliant on behavior to the published [OpAMP specification](https://github.com/open-telemetry/opamp-spec/blob/main/specification.md)

## Building

> This code uses the current stable release of Rust. 

## Support

This library is owned and maintained by members of the CA organization within New Relic. The authors will respond to bug reports and feature requests on a prioritized basis. 

## Contribute

We encourage your contributions to improve otel-opamp-rs! Keep in mind that when you submit your pull request, you'll need to sign the CLA via the click-through using CLA-Assistant. You only have to sign the CLA one time per project.

If you have any questions, or to execute our corporate CLA (which is required if your contribution is on behalf of a company), drop us an email at opensource@newrelic.com.

**A note about vulnerabilities**

As noted in our [security policy](../../security/policy), New Relic is committed to the privacy and security of our customers and their data. We believe that providing coordinated disclosure by security researchers and engaging with the security community are important means to achieve our security goals.

If you believe you have found a security vulnerability in this project or any of New Relic's products or websites, we welcome and greatly appreciate you reporting it to New Relic through [HackerOne](https://hackerone.com/newrelic).

If you would like to contribute to this project, review [these guidelines](./CONTRIBUTING.md).

To all contributors, we thank you!  Without your contribution, this project would not be what it is today.  

## License
OpAMP-RS is licensed under the [Apache 2.0](http://apache.org/licenses/LICENSE-2.0.txt) License.
> The otel-opamp-rs crate also uses source code from third-party libraries. You can find full details on which libraries are used within the Cargo.toml dependency specification section
