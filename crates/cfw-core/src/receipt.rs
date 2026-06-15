pub const RECEIPT_SCHEMA_VERSION: &str = "cfw.receipt.v1";

pub const RECEIPT_SCHEMA_JSON: &str = r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://context-firewall.dev/schemas/receipt.v1.json",
  "title": "Context Firewall Receipt",
  "type": "object",
  "additionalProperties": false,
  "required": [
    "schema_version",
    "spans",
    "raw_estimated_tokens",
    "returned_estimated_tokens",
    "net_estimated_saved",
    "confidence",
    "recent_spans"
  ],
  "properties": {
    "schema_version": {
      "const": "cfw.receipt.v1"
    },
    "spans": {
      "type": "integer",
      "minimum": 0
    },
    "raw_estimated_tokens": {
      "type": "integer",
      "minimum": 0
    },
    "returned_estimated_tokens": {
      "type": "integer",
      "minimum": 0
    },
    "net_estimated_saved": {
      "type": "integer",
      "minimum": 0,
      "description": "Estimated tokens saved by delivery statuses that prove compact output was returned to the agent."
    },
    "confidence": {
      "type": "string",
      "enum": ["low", "medium"]
    },
    "recent_spans": {
      "type": "array",
      "maxItems": 10,
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": [
          "id",
          "kind",
          "raw_estimated_tokens",
          "returned_estimated_tokens",
          "delivery_status",
          "command"
        ],
        "properties": {
          "id": {
            "type": "string",
            "minLength": 1
          },
          "kind": {
            "type": "string",
            "minLength": 1
          },
          "raw_estimated_tokens": {
            "type": "integer",
            "minimum": 0
          },
          "returned_estimated_tokens": {
            "type": "integer",
            "minimum": 0
          },
          "delivery_status": {
            "type": "string",
            "enum": [
              "replaced_tool_result",
              "advisory_wrapper",
              "observed_only",
              "blocked",
              "unknown"
            ]
          },
          "command": {
            "type": ["string", "null"]
          }
        }
      }
    }
  }
}
"#;
