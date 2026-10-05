# Windows USB support

Unruggable uses a USB serial (COM) connection at 115200 baud. The Windows USB sound confirms physical enumeration, but it does not prove that Windows created the COM port the app needs.

## Try this first

1. Use a USB data cable and connect the wallet directly to the PC.
2. Close Arduino, serial terminals, other wallet apps, and anything else that may own the COM port.
3. Open the app's Hardware Wallet screen and select **Rescan Devices**.
4. If the wallet is not listed, expand **Connection details** and copy the diagnostic report.
5. If the report lists an unmatched `COMx`, confirm in Device Manager that it belongs to the wallet, then select **Try COMx**.

The report deliberately omits USB serial numbers.

## If Windows creates no COM port

Open Device Manager and inspect **Ports (COM & LPT)** and **Other devices**. Open the device's Properties and select **Details → Hardware Ids**. Install only the driver matching that ID:

- `USB\VID_303A&PID_1001` — ESP32-S3 native USB Serial/JTAG. Windows normally supplies this automatically.
- `USB\VID_10C4&PID_EA60` — Silicon Labs CP210x VCP driver.
- `USB\VID_1A86&PID_7523` — WCH CH340/CH341 driver.
- `USB\VID_0403&PID_6001` — FTDI VCP driver.

After installation, unplug and reconnect the wallet. Device Manager must show a COM number before the app can communicate with it.

Do not install every driver speculatively. If the Hardware ID does not match one above, send support the copied app diagnostics and the Device Manager Hardware ID.
