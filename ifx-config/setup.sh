#!/bin/bash
# Post-init script — runs once after Informix disk initialization.
# Loads the venueinter database schema and seed data.

source /usr/local/bin/informix_inf.env

echo "Waiting for Informix to come online..."
until onstat -l > /dev/null 2>&1; do
    sleep 2
done

echo "Informix is online. Loading seed data..."
dbaccess - /opt/ibm/config/seed.sql

echo "Seed data loaded successfully."
