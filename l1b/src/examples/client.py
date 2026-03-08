#!/usr/bin/env python2
# -*- coding: utf-8 -*-
# Echo klientas
import socket
import sys

#hostas = '127.0.0.1'                                     # Kur yra mūsų serveris
hostas = '::1'                                            # Lokalus  IPv6
#hostas = '0.0.0.0'                                     # Kur yra mūsų serveris

#portas = int(input('Įvesk porto numerį (>1024) --> '))  # Turi būti tas pats kaip ir serverio
portas = 20000
s = None
for rezultatas in socket.getaddrinfo(hostas, portas, socket.AF_UNSPEC, socket.SOCK_STREAM):
    print('Rezultatas ' + str(rezultatas))
    adresu_seima, soketo_tipas, protokolas, canoninis_vardas, soketo_adresas = rezultatas
    try:
        s = socket.socket(adresu_seima, soketo_tipas, protokolas)
    except OSError as msg:
        s = None
        continue
    try:
        s.connect(soketo_adresas)
    except OSError as msg:
        s.close()
        s = None
        continue
    break
if s is None:
    print('Negaliu atidaryti soketo  :(')
    sys.exit(1)
print('Įvesk eilutę siuntimui ... --> ')
siuntimui = input()  #sys.stdin.readline()
s.send(bytearray(siuntimui,'utf-8'))
gavome = s.recv(1024)
s.close()
print('Gauta ...', str(gavome))
