package validator

import (
	"fmt"
	"strconv"
	"strings"

	"scriptmgr/internal/model"
)

type ValidationError struct {
	Field   string `json:"field"`
	Code    string `json:"code"`
	Message string `json:"message"`
	Value   any    `json:"value,omitempty"`
}

type ValidationErrors []ValidationError

func (e ValidationErrors) Error() string {
	if len(e) == 0 {
		return ""
	}
	parts := make([]string, 0, len(e))
	for _, item := range e {
		parts = append(parts, fmt.Sprintf("%s: %s", item.Field, item.Message))
	}
	return strings.Join(parts, "; ")
}

func ValidateArgs(params []model.ScriptParameter, input []string) ([]string, error) {
	if len(params) == 0 {
		return input, nil
	}

	validated := make([]string, 0, max(len(params), len(input)))
	var errs ValidationErrors

	for i, schema := range params {
		value, ok := argAt(input, i)
		if !ok || strings.TrimSpace(value) == "" {
			if schema.Required && strings.TrimSpace(schema.Default) == "" {
				errs = append(errs, ValidationError{
					Field:   schema.Name,
					Code:    "required",
					Message: fmt.Sprintf("%s is required", schema.Name),
				})
				continue
			}
			value = schema.Default
		}

		if value != "" {
			if err := validateType(schema, value); err != nil {
				errs = append(errs, *err)
				continue
			}
		}
		validated = append(validated, value)
	}

	if len(input) > len(params) {
		validated = append(validated, input[len(params):]...)
	}

	if len(errs) > 0 {
		return nil, errs
	}
	return validated, nil
}

func validateType(schema model.ScriptParameter, value string) *ValidationError {
	switch strings.ToLower(strings.TrimSpace(schema.Type)) {
	case "", "string":
		return nil
	case "integer", "int":
		if _, err := strconv.Atoi(value); err != nil {
			return &ValidationError{
				Field:   schema.Name,
				Code:    "type",
				Message: "expected integer",
				Value:   value,
			}
		}
	case "number", "float":
		if _, err := strconv.ParseFloat(value, 64); err != nil {
			return &ValidationError{
				Field:   schema.Name,
				Code:    "type",
				Message: "expected number",
				Value:   value,
			}
		}
	case "boolean", "bool":
		if _, err := strconv.ParseBool(value); err != nil {
			return &ValidationError{
				Field:   schema.Name,
				Code:    "type",
				Message: "expected boolean",
				Value:   value,
			}
		}
	}
	return nil
}

func argAt(values []string, index int) (string, bool) {
	if index < 0 || index >= len(values) {
		return "", false
	}
	return values[index], true
}

func max(a, b int) int {
	if a > b {
		return a
	}
	return b
}
