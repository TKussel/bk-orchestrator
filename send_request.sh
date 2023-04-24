#!/bin/bash
curl -v -X POST -H "Authorization: ApiKey executor.tobias-develop.broker.dev.ccp-it.dktk.dkfz.de SecretApiKey" --json '
{
  "id": "70c0aa90-bfcf-4312-a6af-42cbd57dc0b8",
  "from": "executor.tobias-develop.broker.dev.ccp-it.dktk.dkfz.de",
  "to": [
    "executor.tobias-develop.broker.dev.ccp-it.dktk.dkfz.de"
  ],
  "body": "{\"executor\":{\"name\":\"BKExecutor\"},\"workflow\":{\"output\":[\"output.csv\"],\"steps\":[{\"name\":\"bk-import\",\"image\":\"beispielimage:develop\",\"env\":[],\"input\":null,\"output\":\"data1.csv\"},{\"name\":\"second-import\",\"image\":\"beispielimage2:develop\",\"env\":[],\"input\":null,\"output\":\"data2.csv\"},{\"name\":\"merge-data\",\"image\":\"merger:develop\",\"env\":[],\"input\":[\"data1.csv\",\"data2.csv\"],\"output\":\"merged.csv\"},{\"name\":\"output-as-pdf\",\"image\":\"printer:develop\",\"env\":[],\"input\":[\"merged.csv\"],\"output\":\"output.csv\"}]}}",
  "failure_strategy": {
    "retry": {
      "backoff_millisecs": 1000,
      "max_tries": 5
    }
  },
  "ttl": "10s",
  "metadata": "The broker can read and use this field e.g., to apply filters on behalf of an app"
}' http://127.0.0.1:8081/v1/tasks
