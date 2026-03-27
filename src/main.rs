use colored::Colorize;
use seraphim::types::{Event, Level, Value};

#[cfg(feature = "iroh")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{collections::HashMap, env::args};

    use colored::Colorize;
    use futures_util::{SinkExt, StreamExt};
    use iroh::{Endpoint, EndpointAddr, endpoint::presets::N0};
    use postcard::from_bytes;
    use tokio::select;
    use tokio_util::codec::{FramedRead, FramedWrite};

    use seraphim::net::{ALPN, ClientProtocol, Request, Response};

    let Some(server_addr) = args().nth(1) else {
        println!(
            "{}",
            "Error: please specify the node address of the server.".red(),
        );
        return Ok(());
    };

    let server_addr = from_bytes::<EndpointAddr>(&z32::decode(server_addr.as_bytes())?)?;

    let ep = Endpoint::bind(N0).await?;
    let conn = ep.connect(server_addr, ALPN).await?;
    let (send, recv) = conn.open_bi().await?;
    let (mut send, mut recv) = (
        FramedWrite::new(send, ClientProtocol),
        FramedRead::new(recv, ClientProtocol),
    );

    send.send(&Request::Status).await?;
    let Some(res) = recv.next().await else {
        println!(
            "{}",
            "Error: server closed the connection before sending its status".red(),
        );
        return Ok(());
    };
    let Response::Status { end } = res? else {
        println!(
            "{}",
            "Error: server sent wrong `{:?}` response instead of status".red(),
        );
        return Ok(());
    };

    send.send(&Request::Read {
        start: 0,
        stop: end,
    })
    .await?;
    let events = loop {
        let Some(res) = recv.next().await else {
            println!(
                "{}",
                "Error: server closed the connection before sending backlog".red(),
            );
            return Ok(());
        };
        let res = res?;
        let Response::Read { events, start } = res else {
            continue;
        };
        if start != 0 {
            println!("{}", "Error: incorrect start index returned".red());
            return Ok(());
        }
        break events;
    };

    let mut callsites = HashMap::<u64, Event>::new();

    for (i, event) in events.iter().enumerate() {
        if let Event::Callsite { .. } = event {
            callsites.insert(i as u64, event.clone());
        } else if let Event::Event {
            parent: _,
            callsite,
            values,
        } = event
        {
            let Some(callsite) = callsites.get(callsite) else {
                continue;
            };
            println!("{}", display_event(callsite, values));
        }
    }

    println!(
        "{}{}{}{}",
        end.to_string().white().bold(),
        " events recorded including ".white(),
        callsites.len().to_string().white().bold(),
        " call sites.".white(),
    );

    loop {
        select! {
            res = recv.next() => {
                let Some(res) = res else {
                    println!("{}", "Connection closed.".white());
                    break;
                };
                let res = match res {
                    Ok(res) => res,
                    Err(err) => {
                        println!("{}", format!("Error: {err}").red());
                        return Ok(());
                    }
                };
                let Response::Update { events, .. } = res else {
                    continue;
                };
                for (i, event) in events.iter().enumerate() {
                    if let Event::Callsite { .. } = event {
                        callsites.insert(i as u64, event.clone());
                    } else if let Event::Event {
                        parent: _,
                        callsite,
                        values,
                    } = event
                    {
                        let Some(callsite) = callsites.get(callsite) else {
                            continue;
                        };
                        println!("{}", display_event(callsite, values));
                    }
                }
            }
        }
    }

    Ok(())
}

fn display_event(callsite: &Event, values: &[Value]) -> String {
    let Event::Callsite {
        name: _,
        target,
        level,
        file: _,
        line: _,
        module_path: _,
        fields,
        kind: _,
    } = callsite
    else {
        panic!("only callsites should be inserted into the callsite map");
    };

    let level = match level {
        Level::Error => "ERROR".red(),
        Level::Warn => "WARN ".yellow(),
        Level::Info => "INFO ".green(),
        Level::Debug => "DEBUG".blue(),
        Level::Trace => "TRACE".purple(),
    };

    if let Some(message) = fields
        .iter()
        .enumerate()
        .find(|(_, f)| *f == "message")
        .and_then(|(i, _)| values.get(i))
    {
        format!("{} {} {} {}", level, target.white(), "->".white(), message)
    } else {
        let mut displayed = format!("{} {} {} ", level, target.white(), "->".white());
        for (field, val) in fields.iter().zip(values) {
            displayed.push_str(&format!("{field}={val}"));
        }
        displayed
    }
}

#[cfg(not(feature = "iroh"))]
fn main() {
    println!("Cannot run client as the `net` feature is disabled.");
}
