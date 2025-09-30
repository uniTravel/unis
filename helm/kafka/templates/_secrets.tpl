{{/*
Generate secret password or retrieve one if already created.

Usage:
{{ include "secrets.passwords" (dict "secret" "secret-name" "key" "keyName" "providedValues" (list "path.to.password1" "path.to.password2") "length" 10 "strong" false "honorProvidedValues" false "context" $) }}

Params:
  - secret - String - Required - Name of the 'Secret' resource where the password is stored.
  - key - String - Required - Name of the key in the secret.
  - providedValues - List<String> - Required - The path to the validating value in the values.yaml, e.g: "mysql.password". Will pick first parameter with a defined value.
  - length - int - Optional - Length of the generated random password.
  - strong - Boolean - Optional - Whether to add symbols to the generated random password.
  - context - Context - Required - Parent context.
  - skipB64enc - Boolean - Optional - Default to false. If set to true, no the secret will not be base64 encrypted.
  - skipQuote - Boolean - Optional - Default to false. If set to true, no quotes will be added around the secret.
  - honorProvidedValues - Boolean - Optional - Default to false. If set to true, the values in providedValues have higher priority than an existing secret
The order in which this function returns a secret password:
  1. Password provided via the values.yaml if honorProvidedValues = true
     (If one of the keys passed to the 'providedValues' parameter to this function is a valid path to a key in the values.yaml and has a value, the value of the first key with a value will be returned)
  2. Already existing 'Secret' resource
     (If a 'Secret' resource is found under the name provided to the 'secret' parameter to this function and that 'Secret' resource contains a key with the name passed as the 'key' parameter to this function then the value of this existing secret password will be returned)
  3. Password provided via the values.yaml if honorProvidedValues = false
     (If one of the keys passed to the 'providedValues' parameter to this function is a valid path to a key in the values.yaml and has a value, the value of the first key with a value will be returned)
  4. Randomly generated secret password
     (A new random secret password with the length specified in the 'length' parameter will be generated and returned)

*/}}
{{- define "secrets.passwords" -}}

{{- $password := "" }}
{{- $passwordLength := default 10 .length }}
{{- $providedPasswordKey := include "utils.getKeyFromList" (dict "keys" .providedValues "context" $.context) }}
{{- $providedPasswordValue := include "utils.getValueFromKey" (dict "key" $providedPasswordKey "context" $.context) }}
{{- $secretData := (lookup "v1" "Secret" (include "names.namespace" .context) .secret).data }}
{{- if $secretData }}
  {{- if hasKey $secretData .key }}
    {{- $password = index $secretData .key | b64dec }}
  {{- end -}}
{{- end }}

{{- if and $providedPasswordValue .honorProvidedValues }}
  {{- $password = $providedPasswordValue | toString }}
{{- end }}

{{- if not $password }}
  {{- if $providedPasswordValue }}
    {{- $password = $providedPasswordValue | toString }}
  {{- else }}
    {{- if .strong }}
      {{- $subStr := list (lower (randAlpha 1)) (randNumeric 1) (upper (randAlpha 1)) | join "_" }}
      {{- $password = randAscii $passwordLength }}
      {{- $password = regexReplaceAllLiteral "\\W" $password "@" | substr 5 $passwordLength }}
      {{- $password = printf "%s%s" $subStr $password | toString | shuffle }}
    {{- else }}
      {{- $password = randAlphaNum $passwordLength }}
    {{- end }}
  {{- end -}}
{{- end -}}
{{- if not .skipB64enc }}
{{- $password = $password | b64enc }}
{{- end -}}
{{- if .skipQuote -}}
{{- printf "%s" $password -}}
{{- else -}}
{{- printf "%s" $password | quote -}}
{{- end -}}
{{- end -}}
