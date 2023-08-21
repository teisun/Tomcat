package com.tomcat.exceptions;

import org.springframework.http.HttpStatus;

public class ApiErrorResponse {

    private HttpStatus status;
    private String error_code;
    private String message;
    private String detail;

    // getter and setters


    public HttpStatus getStatus() {
        return status;
    }

    public void setStatus(HttpStatus status) {
        this.status = status;
    }

    public String getError_code() {
        return error_code;
    }

    @Override
    public String toString() {
        return "ApiErrorResponse{" +
                "status=" + status +
                ", error_code='" + error_code + '\'' +
                ", message='" + message + '\'' +
                ", detail='" + detail + '\'' +
                '}';
    }

    //Builder
    public static final class ApiErrorResponseBuilder {
        private HttpStatus status;
        private String error_code;
        private String message;
        private String detail;

        public ApiErrorResponseBuilder() {
        }

        public static ApiErrorResponseBuilder anApiErrorResponse() {
            return new ApiErrorResponseBuilder();
        }

        public ApiErrorResponseBuilder withStatus(HttpStatus status) {
            this.status = status;
            return this;
        }

        public ApiErrorResponseBuilder withErrorCode(String error_code) {
            this.error_code = error_code;
            return this;
        }

        public ApiErrorResponseBuilder withMessage(String message) {
            this.message = message;
            return this;
        }

        public ApiErrorResponseBuilder withDetail(String detail) {
            this.detail = detail;
            return this;
        }

        public ApiErrorResponse build() {
            ApiErrorResponse apiErrorResponse = new ApiErrorResponse();
            apiErrorResponse.status = this.status;
            apiErrorResponse.error_code = this.error_code;
            apiErrorResponse.detail = this.detail;
            apiErrorResponse.message = this.message;
            return apiErrorResponse;
        }
    }
}