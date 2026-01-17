#!/bin/bash
# Deploy the Cost of Funds Lambda function to AWS
#
# Prerequisites:
# - AWS CLI configured with appropriate credentials
# - cargo-lambda installed (pip install cargo-lambda)
#
# Usage:
#   ./deploy-lambda.sh              # Deploy to default region
#   ./deploy-lambda.sh us-west-2    # Deploy to specific region

set -e

FUNCTION_NAME="trellis-cost-of-funds"
REGION="${1:-us-east-1}"
MEMORY_SIZE=1024  # MB - adjust based on performance needs
TIMEOUT=30        # seconds

echo "=== Trellis Cost of Funds Lambda Deployment ==="
echo "Region: $REGION"
echo "Function: $FUNCTION_NAME"
echo ""

# Build the Lambda function
echo "Building Lambda function..."
cargo lambda build --release --bin lambda_handler

# Check if function exists
if aws lambda get-function --function-name "$FUNCTION_NAME" --region "$REGION" 2>/dev/null; then
    echo "Updating existing function..."
    cargo lambda deploy --binary-name lambda_handler "$FUNCTION_NAME" --region "$REGION"
else
    echo "Creating new function..."
    cargo lambda deploy --binary-name lambda_handler "$FUNCTION_NAME" \
        --region "$REGION" \
        --memory-size "$MEMORY_SIZE" \
        --timeout "$TIMEOUT" \
        --env-vars "RUST_LOG=info"
fi

# Create or update function URL for direct HTTP access
echo ""
echo "Configuring function URL..."
FUNCTION_URL=$(aws lambda get-function-url-config --function-name "$FUNCTION_NAME" --region "$REGION" 2>/dev/null | jq -r '.FunctionUrl' || echo "")

if [ -z "$FUNCTION_URL" ] || [ "$FUNCTION_URL" == "null" ]; then
    echo "Creating function URL..."
    aws lambda create-function-url-config \
        --function-name "$FUNCTION_NAME" \
        --auth-type NONE \
        --region "$REGION"

    # Add permission for public access
    aws lambda add-permission \
        --function-name "$FUNCTION_NAME" \
        --statement-id FunctionURLAllowPublicAccess \
        --action lambda:InvokeFunctionUrl \
        --principal "*" \
        --function-url-auth-type NONE \
        --region "$REGION" 2>/dev/null || true
fi

# Get the function URL
FUNCTION_URL=$(aws lambda get-function-url-config --function-name "$FUNCTION_NAME" --region "$REGION" | jq -r '.FunctionUrl')

echo ""
echo "=== Deployment Complete ==="
echo ""
echo "Lambda Function URL:"
echo "  $FUNCTION_URL"
echo ""
echo "To use in your frontend, set this environment variable:"
echo "  LAMBDA_FUNCTION_URL=$FUNCTION_URL"
echo "  USE_LAMBDA=true"
echo ""
echo "Test the function:"
echo "  curl -X POST $FUNCTION_URL \\"
echo "    -H 'Content-Type: application/json' \\"
echo "    -d '{\"projection_months\": 768, \"use_dynamic_inforce\": true}'"
