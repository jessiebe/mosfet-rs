# MOSFET Rust

This project is a fork of the open-source implementation of the Open Agent Management Protocol (OpAMP) client library originally developed by New Relic and originally published at https://github.com/newrelic/otel-opamp-rs. 

The protocol provides a network protocol for remote management of agents, aiming to transmit observability metrics in a reliable and efficient manner. Leveraging the performance and memory safety features inherent to Rust, this fork takes it a step further by making the client library vendor-independend.

The name MOSFET is a nod to the OpAMP terminilogy as used in electrical engineering.

 # Why Fork

This vendor-independent version emerges from several key motivations:

This fork allows us to tailor the software to meet diverse needs that the original version may not have addressed and offers an environment for experimentation with novel features and methodologies, potentially leading to advancements beyond the scope of the original project.

Our vendor-independent stance encourages wider community engagement and contributions, from all vendors and individual open source contributors fostering a robust and superior software project.

This project is not affiliated with New Relic and the fork has been created under the rights granted by the Apache 2.0 License under which the original code has been published.

## Getting Started

 This library is published as a standard crate. To add it to your project, include it as a dependency inside Cargo.toml like so:
```
[dependencies]
mosfet-rs = { version = "0.0.8", features = ["http", "websocket", "extras"] }
```

For more details refer to the [API documentation](https://docs.rs/mosfet-rs/latest/mosfet_rs/)
## Features

This library supports the following capabilities
 - HTTP support
 - Websocket support
 - Gzip compression
 - Low resource consumption

The code references stable releases of the OpAMP protocol protobuf definition [here](https://github.com/open-telemetry/opamp-spec) and aims to be standards compliant on behavior to the published [OpAMP specification](https://github.com/open-telemetry/opamp-spec/blob/main/specification.md)

## Building

This code uses the current stable release of Rust.

To checkout and build the code do the following:
```
git clone --recursive https://gitlab.com/nemski/mosfet-rs.git
cargo build
```

## Support

We do not provide a warranty or guarantee of support from the developers or maintainers of the software. The users do not have a right to receive support. This software is provided "as-is".
We do not guarantee that the software will work perfectly or will be free from bugs. We are not liable for any issues or damages that might occur as a result of using the software.

## Contribute

We encourage your contributions to improve mosfet-rs! 


## License
MOSFET-RS is licensed under the [Apache 2.0](http://apache.org/licenses/LICENSE-2.0.txt) License.
> The mosfet-rs crate also uses source code from third-party libraries. You can find full details on which libraries are used within the Cargo.toml dependency specification section
