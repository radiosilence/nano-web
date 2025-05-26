package main

import (
	"bytes"
	"encoding/json"
	"strings"
	"text/template"
)

type TemplateData struct {
	Env         map[string]string `json:"env"`
	Json        string            `json:"json"`
	EscapedJson string            `json:"escapedJson"`
}

func templateRoute(name string, content []byte) ([]byte, error) {
	tmpl, err := template.New(name).Parse(string(content))
	if err != nil {
		return nil, err
	}

	jsonString, err := json.Marshal(appEnv)
	if err != nil {
		return nil, err
	}

	buffer := bufferPool.Get().(*bytes.Buffer)
	defer func() {
		buffer.Reset()
		bufferPool.Put(buffer)
	}()

	err = tmpl.Execute(buffer, &TemplateData{
		Env:         appEnv,
		Json:        string(jsonString),
		EscapedJson: strings.Replace(string(jsonString), "\"", "\\\"", -1),
	})
	if err != nil {
		return nil, err
	}

	result := make([]byte, buffer.Len())
	copy(result, buffer.Bytes())
	return result, nil
}