e anal.timeout=60
aaa
echo === SECTION CHECK ===
s 0x00455190; iS~.text
s 0x00517950; iS~.text
s 0x0043CDA0; iS~.text
s 0x0062B5D0; iS~.text
s 0x00401000; iS~.text
s 0x00401010; iS~.text
s 0x0106CDCC; iS~.data
echo === FUNCTION XREFS ===
s 0x00455190; afi~name,nbbs,xref
s 0x00517950; afi~name,xref
s 0x0043CDA0; afi~name,xref
s 0x0062B5D0; afi~name,xref
echo === XREF COUNTS ===
axt 0x00455190~:~
axt 0x00517950~:~
axt 0x0043CDA0~:~
axt 0x0062B5D0~:~
echo === DISASM ===
s 0x00455190; pd 8
s 0x00517950; pd 6
s 0x0043CDA0; pd 6
s 0x0062B5D0; pd 6
q
