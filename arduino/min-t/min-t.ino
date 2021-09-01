//
// Example program for the Arduino M0 Pro (based on Atmel SAMD21)
//
// Uses Serial for programming the board and debug printing, and SerialUSB for
// running the MIN protocol. A programming running on a PC in Python is used to
// exercise this code.
//
// The example does the following:
//
// Every 1 second it sends a MIN frame with ID 51 (0x33) using the transport protocol.
// Every incoming MIN frame is echoed back with the same payload but an ID 1 bigger.
//
// See also the Python program that runs on the host.

// This is an easy way to bring the MIN code into an Arduino project. It's better
// to use a Makefile or IDE project file if the application is to be written in C.

#include "min.h"
#include "min.c"

// A MIN context (we only have one because we're going to use a single port).
// MIN 2.0 supports multiple contexts, each on a separate port, but in this example
// we will use just SerialUSB.
struct min_context min_ctx;

// This is used to keep track of when the next example message will be sent
uint32_t last_sent;

////////////////////////////////// CALLBACKS ///////////////////////////////////

void min_tx_start(uint8_t port){

}

void min_tx_finished(uint8_t port) {
}

// Tell MIN how much space there is to write to the serial port. This is used
// inside MIN to decide whether to bother sending a frame or not.
uint16_t min_tx_space(uint8_t port)
{
  return 255;
}

// Send a character on the designated port.
void min_tx_byte(uint8_t port, uint8_t byte)
{
  // Ignore 'port' because we have just one context.
  Serial.write(&byte, 1U);  
}

// Tell MIN the current time in milliseconds.
uint32_t min_time_ms(void)
{
  return millis();
}

void min_application_handler(uint8_t min_id, uint8_t const *min_payload, uint8_t len_payload, uint8_t port)
{
}

void setup() {
  // put your setup code here, to run once:
  Serial.begin(115200);
  while(!Serial) {
    ; // Wait for serial port
  }

  // Initialize the single context. Since we are going to ignore the port value we could
  // use any value. But in a bigger program we would probably use it as an index.
  min_init_context(&min_ctx, 0);

  last_sent = millis();
}

uint8_t min_payload[128] = {0};

void loop() {
  char buf[32];
  size_t buf_len;

  // Read some bytes from the USB serial port..
  if(Serial.available() > 0) {
    buf_len = Serial.readBytes(buf, 32U);
  }
  else {
    buf_len = 0;
  }
  // .. and push them into MIN. It doesn't matter if the bytes are read in one by
  // one or in a chunk (other than for efficiency) so this can match the way in which
  // serial handling is done (e.g. in some systems the serial port hardware register could
  // be polled and a byte pushed into MIN as it arrives).
  min_poll(&min_ctx, (uint8_t *)buf, (uint8_t)buf_len);

  // Every 1s send a MIN frame using the reliable transport stream.
  uint32_t now = millis();
  // Use modulo arithmetic so that it will continue to work when the time value wraps
  if (now - last_sent > 1000U) {
    // Send a MIN frame with ID 0x33 (51 in decimal) and with a 4 byte payload of the 
    // the current time in milliseconds. The payload will be in this machine's
    // endianness - i.e. little endian - and so the host code will need to flip the bytes
    // around to decode it. It's a good idea to stick to MIN network ordering (i.e. big
    // endian) for payload words but this would make this example program more complex.
    uint8_t n = sprintf(min_payload, "{\"id\":1,\"name\":\"SN-001\",\"temperature\": 27.45,\"humidity\": 25.36,\"voltage\": 3.88,\"status\": 0}");
    if(!min_queue_frame(&min_ctx, 0x33U, min_payload, n)) {
      // The queue has overflowed for some reason
    }
    last_sent = now;
  }
}


