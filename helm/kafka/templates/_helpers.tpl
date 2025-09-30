{{/*
Common labels
*/}}
{{- define "kafka.labels" -}}
helm.sh/chart: {{ include "names.chart" . }}
{{ include "kafka.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "kafka.selectorLabels" -}}
app.kubernetes.io/name: {{ include "names.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Return the Kafka configuration configmap
*/}}
{{- define "kafka.configmapName" -}}
{{- printf "%s-configuration" (include "names.fullname" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{/*
Returns the Kafka listeners settings based on the listeners.* object
*/}}
{{- define "kafka.listeners" -}}
{{- $res := list -}}
{{- range $listener := .Values.listeners -}}
{{- $res = append $res (printf "%s://:%d" (upper $listener.name) (int $listener.containerPort)) -}}
{{- end -}}
{{- printf "%s" (join "," $res) -}}
{{- end -}}

{{/*
Returns the list of advertised listeners, although the advertised address will be replaced during each node init time
*/}}
{{- define "kafka.advertisedListeners" -}}
{{- $listeners := list .Values.listeners.client .Values.listeners.interbroker -}}
{{- $res := list -}}
{{- range $listener := $listeners -}}
{{- $res = append $res (printf "%s://advertised-address-placeholder:%d" (upper $listener.name) (int $listener.containerPort)) -}}
{{- end -}}
{{- printf "%s" (join "," $res) -}}
{{- end -}}

{{/*
Returns the value listener.security.protocol.map based on the values of 'listeners.*.protocol'
*/}}
{{- define "kafka.securityProtocolMap" -}}
{{- $res := list -}}
{{- range $listener := .Values.listeners -}}
{{- $res = append $res (printf "%s:%s" (upper $listener.name) (upper $listener.protocol)) -}}
{{- end -}}
{{ printf "%s" (join "," $res)}}
{{- end -}}

{{/*
Returns the controller quorum voters
*/}}
{{- define "kafka.kraft.controllerQuorumVoters" -}}
{{- $controllerVoters := list -}}
{{- $fullname := include "names.fullname" . -}}
{{- $releaseNamespace := include "names.namespace" . -}}
{{- range $i := until (int .Values.replicaCount) -}}
{{- $nodeId := add (int $i) 0 -}}
{{- $nodeAddress := printf "%s-%d.%s-headless.%s.svc.cluster.local:%d" $fullname (int $i) $fullname $releaseNamespace (int $.Values.listeners.controller.containerPort) -}}
{{- $controllerVoters = append $controllerVoters (printf "%d@%s" $nodeId $nodeAddress ) -}}
{{- end -}}
{{- join "," $controllerVoters -}}
{{- end -}}

{{/*
Init container definition for Kafka initialization
*/}}
{{- define "kafka.prepareKafkaInitContainer" -}}
- name: kafka-init
  image: "{{ .Values.image.repository }}:{{ .Values.image.tag | default "latest" }}"
  imagePullPolicy: {{ .Values.image.pullPolicy }}
  command: ["bash", "-c"]
  args:
    - |
      /scripts/kafka-init.sh
  env:
    - name: POD_NAME
      valueFrom:
        fieldRef:
          fieldPath: metadata.name
  volumeMounts:
    - name: kafka-config
      mountPath: /config
    - name: kafka-configmaps
      mountPath: /configmaps
    - name: scripts
      mountPath: /scripts
{{- end -}}
