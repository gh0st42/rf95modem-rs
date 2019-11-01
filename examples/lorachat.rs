// Based upon RustyChat by Samba Diallo
// https://github.com/SambaDialloB/RustyChat
//
// Adapted to use rf95modem and lora for communication

use cursive::align::HAlign;
use cursive::traits::*;
use cursive::Cursive;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use cursive::view::ScrollStrategy;
use cursive::views::{BoxView, Dialog, DummyView, EditView, LinearLayout, ScrollView, TextView};

use rf95modem::{get_default_usb_serial, RF95modem};
use rf95modem::loradev::RF95LoraDevice;

#[macro_use]
extern crate lazy_static;
use std::sync::Mutex;

lazy_static! {
    // Channels for sending out chat messages
    static ref OUTGOING: (Mutex<Sender<String>>, Mutex<Receiver<String>>) = {
        let (a, b) = channel();
        (Mutex::new(a), Mutex::new(b))
    };
    // Channels for receiving chat messages
    static ref INCOMING: (Mutex<Sender<String>>, Mutex<Receiver<String>>) = {
        let (a, b) = channel();
        (Mutex::new(a), Mutex::new(b))
    };
    static ref MODEM: Mutex<RF95modem> = {
        let device = get_default_usb_serial();
        Mutex::new(RF95modem::new(&device, 115_200))
    };
}
fn main() {
    MODEM.lock().unwrap().open().unwrap();

    let title = "rf95modem LoRa Chat";

    // Channel to set channel name, pun intended :)
    let (channel_sender, channel_receiver) = channel();

    //Create a separate thread, this allows us to have a subscribe loop that wont stop the UI from updating
    let _handle1 = thread::spawn(move || {
        println!("Subscribed to channel. Enter messages to publish!");
        //We wait for the UI to send us the channel name
        let test_channel = channel_receiver.recv();
        if test_channel.is_ok() {
            let channel_name: String = test_channel.unwrap();
            let msg_sender = INCOMING.0.lock().unwrap().clone();
            //Once we have the channel name, we create a loop that lets us request messages
            loop {
                if let Ok(pkt) = MODEM.lock().unwrap().read_packet() {
                    let data = std::str::from_utf8(&pkt.data).unwrap();
                    if data.contains('|') {
                        let fields: Vec<&str> = data.split('|').collect();
                        if fields[0] == channel_name {
                            msg_sender.send(fields[1..].join("|")).unwrap();
                        }
                    }
                }
                let delay = std::time::Duration::from_millis(50);
                thread::sleep(delay);
            }
        }
    });
    //Create a separate thread, this allows us to have a sender loop that wont stop the UI from updating
    let _handle2 = thread::spawn(move || {
        println!("Sender loop running!");
        let out_receiver = OUTGOING.1.lock().unwrap();
        //We wait for the UI to send us the channel name
        loop {
            let data = out_receiver.recv();
            send_data(&mut MODEM.lock().unwrap(), data.unwrap());
        }
    });

    // Creates the cursive root - required for every application.
    let mut siv = Cursive::default();
    //First layer - get username and channel
    siv.add_layer(
        Dialog::around(
            LinearLayout::vertical()
                .child(DummyView.fixed_height(1))
                .child(TextView::new("Enter Username").h_align(HAlign::Center))
                .child(EditView::new().with_id("username").fixed_width(20))
                .child(DummyView.fixed_height(1))
                .child(TextView::new("Enter Channel").h_align(HAlign::Center))
                .child(EditView::new().with_id("channel").fixed_width(20)),
        )
        .title(title)
        .button("Okay", move |s| {
            //Saving inputs content to variables to check them.
            let channel = s
                .call_on_id("channel", |view: &mut EditView| view.get_content())
                .unwrap();
            let username = s
                .call_on_id("username", |view: &mut EditView| view.get_content())
                .unwrap();
            //Checking if either input is empty.

            if username.is_empty() {
                s.add_layer(Dialog::info("Please enter a username !".to_string()));
            } else {
                let new_channel = if channel.is_empty() {
                    "global".to_string()
                } else {
                    channel.to_string()
                };
                channel_sender.send(new_channel).unwrap();
                s.pop_layer();
                s.add_layer(BoxView::with_fixed_size(
                    (40, 20),
                    Dialog::new()
                        .title(title)
                        .content(
                            //Instead of using a ListView, we use a ScrollView with a LinearLayout inside.
                            //This allows us to remove the extra lines from the View
                            LinearLayout::vertical()
                                .child(
                                    ScrollView::new(
                                        LinearLayout::vertical()
                                            .child(DummyView.fixed_height(1))
                                            //Add in a certain amount of dummy views, to make the new messages appear at the bottom
                                            .with(|messages| {
                                                for _ in 0..13 {
                                                    messages.add_child(DummyView.fixed_height(1));
                                                }
                                            })
                                            .child(DummyView.fixed_height(1))
                                            .with_id("messages"),
                                    )
                                    .scroll_strategy(ScrollStrategy::StickToBottom),
                                )
                                .child(EditView::new().with_id("message")),
                        )
                        .h_align(HAlign::Center)
                        .button("Send", move |s| {
                            let message = s
                                .call_on_id("message", |view: &mut EditView| view.get_content())
                                .unwrap();
                            let new_channel_2 = if channel.is_empty() {
                                "global".to_string()
                            } else {
                                channel.to_string()
                            };
                            if message.is_empty() {
                                s.add_layer(
                                    Dialog::new()
                                        .title(title)
                                        .content(TextView::new("Please enter a message!!"))
                                        .button("Okay", |s| {
                                            s.pop_layer();
                                        }),
                                )
                            } else {
                                publish(message.to_string(), username.to_string(), new_channel_2);
                                let msg_sender = INCOMING.0.lock().unwrap().clone();
                                msg_sender
                                    .send(format!("> {}", message.to_string()))
                                    .unwrap();
                                //Clear out the EditView.
                                s.call_on_id("message", |view: &mut EditView| view.set_content(""))
                                    .unwrap();
                            }
                        })
                        .button("Quit", |s| s.quit()),
                ));
            }
        })
        .button("Quit", |s| s.quit())
        .h_align(HAlign::Center),
    );
    //This is where we check for updates from the subscribe function.
    //We have a message count  and a loop, refreshing whenever there is a new message coming in.
    let mut message_count = 0;
    siv.refresh();
    loop {
        siv.step();
        if !siv.is_running() {
            break;
        }

        let msg_receiver = INCOMING.1.lock().unwrap();
        let mut needs_refresh = false;
        //Non blocking channel receiver.
        for m in msg_receiver.try_iter() {
            siv.call_on_id("messages", |messages: &mut LinearLayout| {
                needs_refresh = true;
                message_count += 1;
                messages.add_child(TextView::new(m.to_string()));
                if message_count <= 14 {
                    messages.remove_child(0);
                }
            });
        }
        if needs_refresh {
            siv.refresh();
        }
    }
}

fn publish(text: String, username: String, channel: String) {
    let message = format!("{}|{}|{}", channel, username, text);
    let out_sender = OUTGOING.0.lock().unwrap().clone();
    out_sender.send(message).unwrap();
}

fn send_data(modem: &mut RF95modem, data: String) {
    modem.send_data(data.as_bytes().to_vec()).unwrap();
}
