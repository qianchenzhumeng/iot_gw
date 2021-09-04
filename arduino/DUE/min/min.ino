#define NO_TRANSPORT_PROTOCOL

#include "min.h"
#include "min.c"

// A MIN context (we only have one because we're going to use a single port).
// MIN 2.0 supports multiple contexts, each on a separate port, but in this example
// we will use just Serial1.
struct min_context min_ctx;

////////////////////////////////// CALLBACKS ///////////////////////////////////

void min_tx_start(uint8_t port){}

void min_tx_finished(uint8_t port) {}

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
  Serial1.write(&byte, 1U);  
}

void min_application_handler(uint8_t min_id, uint8_t const *min_payload, uint8_t len_payload, uint8_t port)
{
  char msg[32] = {0};
  char *turn_on = "turn_on";
  char *turn_off = "turn_off";
  Serial.print("MIN frame with ID ");
  Serial.print(min_id);
  Serial.print(" received at ");
  Serial.println(millis());

  memset(msg, 0, sizeof(msg));
  snprintf(msg, len_payload < sizeof(msg) ? (len_payload+1) : sizeof(msg), "%s", min_payload);
  Serial.println(msg);
  if(0 == strncmp(turn_on, msg, sizeof(msg))) {
    digitalWrite(LED_BUILTIN, HIGH);
  } else if(0 == strncmp(turn_off, msg, sizeof(msg))) {
    digitalWrite(LED_BUILTIN, LOW);    
  }
}

void setup() {
  pinMode(LED_BUILTIN, OUTPUT);
  Serial.begin(115200);
  Serial1.begin(115200);
  while(!Serial) {
    ; // Wait for serial port
  }
  // Wait for USB serial port to connect. Note that this won't return until the host PC
  // opens the USB serial port.
  while(!Serial1) {
    ;
  }
  // Initialize the single context. Since we are going to ignore the port value we could
  // use any value. But in a bigger program we would probably use it as an index.
  min_init_context(&min_ctx, 0);
  last_sent = millis();
}

uint8_t min_payload[128] = {0};

void loop() {
  char buf[32];
  size_t buf_len, n;

  // Read some bytes from the USB serial port..
  if(Serial1.available() > 0) {
    buf_len = Serial1.readBytes(buf, 32U);
    min_poll(&min_ctx, (uint8_t *)buf, (uint8_t)buf_len);
  }  

  uint32_t now = millis();
  if (now - last_sent > 5000U) {
    n = snprintf((char *)min_payload, sizeof(min_payload), "{\"id\":1,\"name\":\"SN-001\",\"temperature\": 27.45,\"humidity\": 25.36,\"voltage\": 3.88,\"status\": 0}");
    min_send_frame(&min_ctx, 0x33U, min_payload, n);
    last_sent = now;
  }
}


