You're building a new Rust based project called "photoframe-rs"

The main crate is in a subdirectory called "photoframe-server". The idea is that I'll expand this later with separate crates for the microcontroller implementations. The main crate should contain a simple program that acts as a long running server process. The server should be configurable (use the `config` crate), and use an async runtime (use `tokio` here).

The server is responsible for updating what photo is displayed on a number of spectra e6 e-ink displays, which can be performed by sending a HTTP POST to each photo frame containing the image data in bmp format.

The photo frames should be configurable in the server's config file, with a `photoframes` section containing the following information about each photo frame:

- IP address
- Whether the picture frame should display landscape or portrait pictures
- Width/height of the panel in pixels (this will equal to the submitted bmp files dimensions regardless of overscan settings)
- Overscan parameters (probably in the form of "padding" parameters in pixels from left/right/top/bottom) in case the display is slightly larger than the opening in the photo frame (this should just set the padded area to white pixels on the submitted bmp file, and adjust how the image is scaled to fit the visible area)
- Which method to use for scaling the image so that it fits the visible area (similar to css object fit "cover" or "contain" values)
- Which sources the picture frame should fetch images from (array of source ID:s)
- How often the picture frame image should be updated (use `tokio-cron-scheduler` here)
- Which colors does the panel support
- Post processing parameters for the image (brightness, contrast, sharpness, saturation adjustments). You can use the `photon_rs` crate here
- What dithering algorithm to use (please use the `dither` crate here)

The server configuration should also contain a `sources` section that has different types of sources. For now we can configure either:

- Filesystem directory lookup using glob patterns (use the `glob` crate here)
- Lookup from a Google Photos album via Google Photos API

For each source we should be able to configure:

- Order in which photos are selected (sequential or random (default))
- Source ID which photo frame configurations can refer to
- Any necessary details for the source to function (e.g. the filesystem glob path or API credentials)

When selecting the next image for a photo frame, we need to make sure it's orientation matches the configured orientation of the photo frame. If there's a mismatch, we should try again with the next image. Please try to make use of file/API metadata calls here to figure out orientation whenever possible.

Please use readily available crates where possible instead of reinventing the wheel. Try to keep the implementation simple.

If possible, I'd like a simple implementation of a web UI as well where the configuration can be edited, all configured photoframes can be listed, for each photoframe the current state (showing the final rendered bmp file) of the photoframe can be viewed, and custom images can be uploaded to the photoframe (these should still run through the same resize/crop/padding/dithering/adjustments etc pipeline.) A killer feature here would be to have an instant preview in the web UI that immediately updates a web UI only preview of the currently displayed image when adjusting dithering and image adjustment parameters. So in other words you should keep the currently displayed image in the process memory so that it can be live previewed with different adjustments in the web UI.

Please follow best practices in the project where possible, taking into account the mentioned future prospects of the project. Prefer simple, minimalistic solutions where possible.

# Rust code guidelines:

- When using libraries, check current library version from Cargo.toml and use context7 to look up documentation for the exact version.
- When writing playwright tests, use the playwright MCP.
- Make use of ? syntax where possible.
- API handlers should use struct responses annotated with `#[ts(export)]`
- Avoid using `map_err`, use anyhow `with_context` instead.

# TypeScript code guidelines:

- Don't use React.FC
- Avoid large try/catch blocks. Only wrap code that is expected to throw an exception. Prefer writing many try/catch blocks to handle specific exceptions.
- Don't use `any` types. Prefer type guards over type assertions.
- Prefer using named imports/exports over default imports/exports.
- Avoid large blocks of logic inside React components.
- Prefer using modern JavaScript features

# General guidelines:

- Don't write unnecessary or obvious comments that are self-explanatory.
- Provide brutally honest and realistic assessments of requests, feasibility, and potential issues. No sugar-coating. No vague possibilities where concrete answers are needed.
- If you are unsure about how something works, go read more code. If you can't find the answer, ask for help.
- The API uses camelCase thanks to serde rename_all attribute.
- Prefer code reuse over duplication. Almost identical code blocks should be refactored into functions.
- Think hard
- Prefer functional programming over imperative programming.
- Prefer clear, small pure functions over long listings of code.
- When running CLI commands, avoid using commands that require user interaction (e.g. use git --no-pager, playwright test --reporter=line, etc.)
- There are configuration examples and documentation for the program available under ~/radiator-server-doc
- Follow the existing code style and conventions consistently.
- Write rustdoc and TSDoc comments for public functions.
- **DO NOT** write comments explaining fixes to issues introduced earlier in the same session.
- Don't make functions that only call another function.
- Don't worry about backwards compatibility, as this is a greenfield project.