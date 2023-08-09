package com.tomcat.service;

import com.tomcat.controller.request.AuthenticationRequest;
import com.tomcat.controller.response.AuthenticationResponse;
import com.tomcat.domain.User;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.RequestBody;

import java.util.Optional;

public interface UserService {
  AuthenticationResponse register(AuthenticationRequest request);
  
  AuthenticationResponse login(AuthenticationRequest request);

  Optional<User> findByUsername(String username);

  AuthenticationResponse registerOrLogin(AuthenticationRequest request);
}