# High-level Data Transfer Protocol(HDTP)

Derive from `High-level Data Link Control (HDLC)`.

Frame:

|     Flag      | Length  | Payload |   FCS   |
| :-----------: | :-----: | :-----: | :-----: |
| 1 octet(0x7E) | 1 octet | n octet | 2 octet |

- **Flag** - It is an 8-bit sequence that marks the beginning and the end of the frame. The bit pattern of the flag is 01111110(0x7E in hexadecimal notation). 
- **Length** - It is 1-octet long and specifies the total number of octets contained in the message(including *Length* field itself and *Payload* field). 
- **Payload** - This carries the data.
- **FCS** - It is 2-octet frame check sequence for error detection(CRC-16/XModem). This is only for the **Payload** field.

~~The flag 0x7E is called "byte stuffing". There is a "control escape octet", which has the value 0x7D. If either of these tow octets appears in the transmitted data, an escape octet is sent, followed by the original data octet. For example, the byte 0x7E would be transmitted as 0x7D 0x7E("0111_1101_0111_1110"), and the byte 0x7D would be transmitted as 0x7D 0x7D("0111_1101_0111_1101").~~

