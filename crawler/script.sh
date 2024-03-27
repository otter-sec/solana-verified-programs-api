#!/bin/bash

echo "Running the main script..." >> logs
./crawler

service cron start
echo "* */8 * * * DATABASE_URL=$DATABASE_URL /crawler >> /logs 2>&1" | crontab -

echo "Cronjob activated..." >> logs

# Keep the container running
exec tail -f /dev/null