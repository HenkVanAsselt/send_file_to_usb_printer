// Send a file to a Windows (USB) printer

use std::io::{self, Read};
use std::fs::File;
use std::ptr;
// use winapi::shared::minwindef::{DWORD, LPBYTE};
use winapi::um::winspool::{EnumPrintersW, PRINTER_INFO_2W};
use windows::Win32::Graphics::Printing::*;
use widestring::U16CString;

fn read_utf8_file(file_path: &str) -> io::Result<String> {
    let mut file = File::open(file_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}


// The following is borrowed from https://docs.rs/raw-printer/latest/
#[cfg(target_os = "windows")]
pub fn write_to_device(printer: &str, payload: &str) -> Result<usize, io::Error> {
    use std::ffi::CString;
    use std::ptr;
    // use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Graphics::Printing::{
        ClosePrinter, EndDocPrinter, EndPagePrinter, OpenPrinterA, StartDocPrinterA,
        StartPagePrinter, WritePrinter, DOC_INFO_1A, PRINTER_ACCESS_USE, PRINTER_DEFAULTSA, PRINTER_HANDLE,
    };

    let printer_name = CString::new(printer).unwrap_or_default(); // null-terminated string
    let mut printer_handle: PRINTER_HANDLE = PRINTER_HANDLE::default();

    // Open the printer
    unsafe {
        let pd = PRINTER_DEFAULTSA {
            pDatatype: windows::core::PSTR(ptr::null_mut()),
            pDevMode: ptr::null_mut(),
            DesiredAccess: PRINTER_ACCESS_USE,
        };

        if OpenPrinterA(
            windows::core::PCSTR(printer_name.as_bytes().as_ptr()),
            &mut printer_handle,
            Some(&pd),
        )
            .is_ok()
        {
            let doc_info = DOC_INFO_1A {
                pDocName: windows::core::PSTR("ZPL-Label\0".as_ptr() as *mut u8),
                pOutputFile: windows::core::PSTR::null(),
                pDatatype: windows::core::PSTR("RAW\0".as_ptr() as *mut u8),
            };

            // Start the document
            let job = StartDocPrinterA(printer_handle, 1, &doc_info as *const _ as _);
            if job == 0 {
                return Err(std::io::Error::from(windows::core::Error::from_win32()));
            }

            // Start the page
            if !StartPagePrinter(printer_handle).as_bool() {
                return Err(std::io::Error::from(windows::core::Error::from_win32()));
            }

            let buffer = payload.as_bytes();
            let mut bytes_written: u32 = 0;
            if !WritePrinter(
                printer_handle,
                buffer.as_ptr() as _,
                buffer.len() as u32,
                &mut bytes_written,
            )
                .as_bool()
            {
                return Err(std::io::Error::from(windows::core::Error::from_win32()));
            }

            // End the page and document
            let _ = EndPagePrinter(printer_handle);
            let _ = EndDocPrinter(printer_handle);
            let _ = ClosePrinter(printer_handle);
            Ok(bytes_written as usize)
        } else {
            Err(std::io::Error::from(windows::core::Error::from_win32()))
        }
    }
}

fn enumerate_printers() -> Vec<String>{
    unsafe {
        let mut needed = 0u32;
        let mut returned = 0u32;
        let mut printers: Vec<String> = Vec::new();

        // First, get the required buffer size
        EnumPrintersW(
            PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS,
            ptr::null_mut(),
            2, // PRINTER_INFO_2
            ptr::null_mut(),
            0,
            &mut needed,
            &mut returned,
        );

        if needed == 0 {
            eprintln!("No printers found or failed to get buffer size.");
            return printers
        }

        // Allocate buffer with the required size
        let buffer = vec![0u8; needed as usize];
        let success = EnumPrintersW(
            PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS,
            ptr::null_mut(),
            2, // PRINTER_INFO_2
            buffer.as_ptr() as *mut _,
            needed,
            &mut needed,
            &mut returned,
        );

        if success == 0 {
            eprintln!("Failed to enumerate printers.");
            return printers;
        }

        let printer_info = buffer.as_ptr() as *const PRINTER_INFO_2W;
        for i in 0..returned as isize {
            let pi = printer_info.offset(i);
            let printer_name = if !(*pi).pPrinterName.is_null() {
                U16CString::from_ptr_str((*pi).pPrinterName).to_string_lossy()
            } else {
                String::from("Unknown Printer")
            };

            // println!("Printer: {}", printer_name);
            printers.push(printer_name)
        }
        printers
    }
}

fn main() -> io::Result<()> {

    // Get the printername and (ZPL) filename from the commandline arguments
    let printername = std::env::args().nth(1).expect("No printername given");
    let filename = std::env::args().nth(2).expect("No filename given");

    // Get a vector with available printer names, and check if the given printer is in there.
    // If not found, show the list of available printer names and exit.
    let available_printers = enumerate_printers();
    if available_printers.contains(&printername) {
        // println!("{} is available", printername);
    }
    else {
        println!("'{}' is not available", printername);
        println!("Available printers are:");
        // Print list of available printers
        for (i, prname) in available_printers.iter().enumerate() {
            println!("{}: {}", i+1, prname);
        }
        return Ok(());
    }

    // Read the (ZPL) file and print the contents
    let file_content = read_utf8_file(&filename)?;
    // if file_content.ends_with("\r\n") || file_content.ends_with("\n") {
    //     // All ok
    // }
    // else {
    //     file_content.push_str("\n");
    // }
    // println!("{}", &file_content);

    let bytes_written = write_to_device(&printername, &file_content);
    println!("wrote {} bytes", bytes_written?);

    Ok(())
}