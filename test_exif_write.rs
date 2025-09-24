// Test script to explore kamadak-exif API for writing EXIF data to PNG
use std::io::Cursor;

fn main() {
    // Let's see what's available in the exif crate for writing
    println!("Checking exif crate API...");

    // Create a basic EXIF field
    let datetime_str = "2024:01:15 10:30:45";

    // Try to find how to create EXIF data and write to PNG
    let mut writer = exif::Writer::new();

    // Add a DateTime field
    let field = exif::Field {
        tag: exif::Tag::DateTime,
        ifd_num: exif::In::PRIMARY,
        value: exif::Value::Ascii(vec![datetime_str.as_bytes().to_vec()]),
    };

    writer.push_field(&field);

    // Try to write to a buffer
    let mut buf = Vec::new();
    match writer.write(&mut buf, false) {
        Ok(_) => println!("Successfully created EXIF data"),
        Err(e) => println!("Error creating EXIF data: {}", e),
    }
}
