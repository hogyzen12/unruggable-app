# Windows USB support

Unruggable uses a USB serial (COM) connection at 115200 baud. Hearing the Windows USB sound confirms physical enumeration, but it does not guarantee that Windows created the COM port the app needs.

## Try this first

1. Use a USB data cable and connect the wallet directly to the PC.
2. Close Arduino, serial terminals, other wallet apps, and any program that may own the COM port.
3. Open the app's Hardware Wallet screen and choose **Rescan Devices**.
4. If the device is not listed, expand **Connection details** and copy the diagnostic report.
5. If the report lists an unmatched `COMx`, confirm in Device Manager that it belongs to the wallet, then use **Try COMx**.

The diagnostic report omits USB serial numbers.

## If Windows creates no COM port

Open Device Manager and inspect **Ports (COM & LPT)** and **Other devices**. In the device's Properties, copy **Details → Hardware Ids**. Install only the driver matching that ID:

- `USB\VID_303A&PID_1001` — ESP32-S3 native USB Serial/JTAG. Windows 10 and later normally obtain this automatically while online. See [Espressif USB Serial/JTAG driver guidance](https://docs.espressif.com/projects/esp-iot-solution/en/release-v2.0/usb/usb_overview/usb_serial_jtag.html#usb-serial-jtag-peripheral-driver).
- `USB\VID_10C4&PID_EA60` — Silicon Labs CP210x bridge. Install the [official CP210x VCP driver](https://www.silabs.com/software-and-tools/usb-to-uart-bridge-vcp-drivers).
- `USB\VID_1A86&PID_7523` — WCH CH340/CH341 bridge. Install the [official WCH CH341SER driver](https://www.wch-ic.com/downloads/CH341SER_ZIP.html).
- `USB\VID_0403&PID_6001` — FTDI bridge. Install the [official FTDI VCP driver](https://ftdichip.com/drivers/vcp-drivers/).

After installation, unplug and reconnect the wallet. Device Manager must show a COM number before rescanning in the app.

Do not install every driver speculatively. If the Hardware Id does not match one of the IDs above, send the copied app diagnostics and the Device Manager Hardware Id to support.

## What the app can and cannot repair

The app can identify supported VID/PID pairs, show enumeration/open errors, and try an unmatched COM port explicitly. It cannot communicate when Windows has not created a COM port; that condition must be resolved by the matching Windows driver, cable, hub, or USB policy.
