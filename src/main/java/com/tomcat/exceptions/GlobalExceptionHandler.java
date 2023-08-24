package com.tomcat.exceptions;

import lombok.extern.slf4j.Slf4j;
import org.springframework.http.HttpHeaders;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.http.converter.HttpMessageNotReadableException;
import org.springframework.messaging.handler.annotation.support.MethodArgumentNotValidException;
import org.springframework.messaging.handler.annotation.support.MethodArgumentTypeMismatchException;
import org.springframework.web.HttpRequestMethodNotSupportedException;
import org.springframework.web.bind.annotation.ControllerAdvice;
import org.springframework.web.bind.annotation.ExceptionHandler;
import org.springframework.web.context.request.WebRequest;

import java.nio.file.AccessDeniedException;
import java.security.InvalidParameterException;
import java.util.NoSuchElementException;

@ControllerAdvice
@Slf4j
public class GlobalExceptionHandler {

  @ExceptionHandler(MethodArgumentNotValidException.class)
  public ResponseEntity<ApiErrorResponse> handleValidationException(MethodArgumentNotValidException ex) {
    ApiErrorResponse response = new ApiErrorResponse.ApiErrorResponseBuilder()
            .withStatus(HttpStatus.BAD_REQUEST)
            .withErrorCode("VALIDATION_ERROR")
            .withMessage(ex.getBindingResult().getAllErrors().get(0).getDefaultMessage())
            .build();

    return ResponseEntity.status(response.getStatus()).body(response);
  }

  @ExceptionHandler(AccessDeniedException.class)
  public ResponseEntity<ApiErrorResponse> handleAccessDeniedException() {
    ApiErrorResponse response = new ApiErrorResponse.ApiErrorResponseBuilder()
            .withStatus(HttpStatus.FORBIDDEN)
            .withErrorCode("ACCESS_DENIED")
            .withMessage("You don't have permission to access this resource")
            .build();

    return ResponseEntity.status(response.getStatus()).body(response);
  }

  @ExceptionHandler(NoSuchElementException.class)
  public ResponseEntity<ApiErrorResponse> handleNoSuchElementException(NoSuchElementException ex) {

    ApiErrorResponse response = new ApiErrorResponse.ApiErrorResponseBuilder()
            .withStatus(HttpStatus.NOT_FOUND)
            .withErrorCode("NOT_FOUND")
            .withMessage(ex.getMessage())
            .build();

    return ResponseEntity.status(response.getStatus()).body(response);
  }


  @ExceptionHandler(BusinessException.class)
  public ResponseEntity<ApiErrorResponse> handleBusinessException(BusinessException ex) {
    ApiErrorResponse response = new ApiErrorResponse.ApiErrorResponseBuilder()
            .withStatus(HttpStatus.INTERNAL_SERVER_ERROR)
            .withErrorCode(ex.getErrorCode())
            .withMessage(ex.toString())
            .build();
    log.error("!exception!: " + response.toString());
    return ResponseEntity.status(response.getStatus()).body(response);
  }

  @ExceptionHandler({MethodArgumentTypeMismatchException.class})
  protected ResponseEntity<ApiErrorResponse> handleMethodArgumentNotValid(MethodArgumentNotValidException ex){
    return buildResponseEntity(ex, HttpStatus.BAD_REQUEST, HttpStatus.BAD_REQUEST.name());
  }

  @ExceptionHandler(InvalidParameterException.class)
  public ResponseEntity<ApiErrorResponse> handleInvalidParameter(InvalidParameterException ex) {
    return buildResponseEntity(ex, HttpStatus.BAD_REQUEST, HttpStatus.BAD_REQUEST.name());
  }

  @ExceptionHandler(HttpRequestMethodNotSupportedException.class)
  public ResponseEntity<ApiErrorResponse> handleHttpRequestMethodNotSupportedException(Exception ex) {
    ApiErrorResponse response =new ApiErrorResponse.ApiErrorResponseBuilder()
            .withStatus(HttpStatus.METHOD_NOT_ALLOWED)
            .withErrorCode(HttpStatus.METHOD_NOT_ALLOWED.name())
            .withMessage(ex.getLocalizedMessage())
            .withDetail( ex.toString()).build();
    ResponseEntity<ApiErrorResponse> responseEntity = new ResponseEntity<>(response, response.getStatus());
    log.error("!exception!: " + responseEntity.toString());
    return responseEntity;
  }

  @ExceptionHandler({HttpMessageNotReadableException.class})
  protected ResponseEntity<ApiErrorResponse> handleHttpMessageNotReadable(HttpMessageNotReadableException ex, HttpHeaders headers, HttpStatus status, WebRequest request) {
    String error = "Malformed JSON request ";
    ApiErrorResponse response = new ApiErrorResponse.ApiErrorResponseBuilder()
            .withStatus(status)
            .withErrorCode("BAD_DATA")
            .withMessage(ex.getLocalizedMessage())
            .withDetail(error + ex.getMessage()).build();
    ResponseEntity<ApiErrorResponse> responseEntity = new ResponseEntity<>(response, response.getStatus());
    log.error("!exception!: " + responseEntity.toString());
    return responseEntity;
  }


  @ExceptionHandler(Exception.class)
  public ResponseEntity<ApiErrorResponse> handleNotFound(Exception ex) {
    return buildResponseEntity(ex, HttpStatus.INTERNAL_SERVER_ERROR, HttpStatus.INTERNAL_SERVER_ERROR.name());
  }

  private ResponseEntity buildResponseEntity(Exception ex, HttpStatus status, String error_code){
    ApiErrorResponse response =new ApiErrorResponse.ApiErrorResponseBuilder()
            .withStatus(status)
            .withErrorCode(error_code)
            .withMessage(ex.getMessage())
            .withDetail(ex.toString()).build();
    ResponseEntity<ApiErrorResponse> responseEntity = ResponseEntity.status(response.getStatus()).body(response);
    log.error("!exception!: " + responseEntity.toString());
    return responseEntity;
  }



}