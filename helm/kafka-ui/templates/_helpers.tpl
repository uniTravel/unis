{{/*
Common labels
*/}}
{{- define "kafka-ui.labels" -}}
helm.sh/chart: {{ include "names.chart" . }}
{{ include "kafka-ui.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "kafka-ui.selectorLabels" -}}
app.kubernetes.io/name: {{ include "names.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}
